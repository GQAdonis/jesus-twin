//! The [`Gatekeeper`] trait, its [`Permit`] / [`Cost`] types, and two implementations:
//! [`OpenGatekeeper`] (always admits) and [`SemaphoreGatekeeper`] (bounded concurrency).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Debug, Error)]
pub enum AdmissionError {
    #[error("rejected: system at capacity")]
    Rejected,
    #[error("timed out waiting for admission")]
    Timeout,
    #[error("requested cost {cost} exceeds total capacity {capacity}")]
    CostTooLarge { cost: u32, capacity: usize },
}

/// Estimated resource units a request will consume (maps request size -> units). One unit ==
/// one semaphore permit. The orchestrator passes a `Cost`; the gatekeeper reserves that many
/// units for the lifetime of the [`Permit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost(pub u32);

/// Proof of admission. RAII: the reserved units are released when this is dropped, so a
/// dropped/cancelled request can never leak capacity. An `OpenGatekeeper` permit holds no
/// semaphore units (`None`); a `SemaphoreGatekeeper` permit owns the acquired units.
#[derive(Debug)]
pub struct Permit {
    cost: Cost,
    _units: Option<OwnedSemaphorePermit>,
}

impl Permit {
    /// A permit that reserves nothing (the open/no-backpressure case).
    pub fn unbounded(cost: Cost) -> Self {
        Self { cost, _units: None }
    }

    /// A permit owning `units` semaphore permits; dropping it releases them.
    pub fn with_units(cost: Cost, units: OwnedSemaphorePermit) -> Self {
        Self {
            cost,
            _units: Some(units),
        }
    }

    pub fn cost(&self) -> Cost {
        self.cost
    }
}

/// Decides whether a request enters the inference engine, and when (ARCHITECTURE.md §6).
/// This is admission control only — it never executes the request.
#[async_trait]
pub trait Gatekeeper: Send + Sync {
    /// Acquire a [`Permit`] for a request of the given `cost`, or fail with
    /// [`AdmissionError`] (which an adapter maps to 503 / busy).
    async fn admit(&self, cost: Cost) -> Result<Permit, AdmissionError>;
}

/// A gatekeeper that always admits — no backpressure. The honest default for the edge /
/// single-user build: it satisfies the contract without pretending to limit anything.
#[derive(Debug, Default, Clone)]
pub struct OpenGatekeeper;

#[async_trait]
impl Gatekeeper for OpenGatekeeper {
    async fn admit(&self, cost: Cost) -> Result<Permit, AdmissionError> {
        Ok(Permit::unbounded(cost))
    }
}

/// Bounded-concurrency admission via a tokio [`Semaphore`].
///
/// A request reserves `cost` units (permits). When the semaphore is exhausted, callers wait —
/// but only up to `acquire_timeout`, and only if fewer than `max_queue_depth` callers are
/// already waiting. Over either limit, admission is rejected (the engine surfaces 503). This
/// is the ARCHITECTURE.md §6 contract: decide *whether/when* a request enters the engine;
/// the engine owns *how* admitted requests run.
#[derive(Debug, Clone)]
pub struct SemaphoreGatekeeper {
    semaphore: Arc<Semaphore>,
    capacity: usize,
    waiting: Arc<AtomicUsize>,
    max_queue_depth: usize,
    acquire_timeout: Duration,
}

impl SemaphoreGatekeeper {
    /// `max_units` total in-flight units, `max_queue_depth` waiters allowed before rejecting,
    /// `acquire_timeout` the longest a request waits for capacity before timing out.
    pub fn new(max_units: usize, max_queue_depth: usize, acquire_timeout: Duration) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_units)),
            capacity: max_units,
            waiting: Arc::new(AtomicUsize::new(0)),
            max_queue_depth,
            acquire_timeout,
        }
    }

    /// Currently available units (for stats / tests).
    pub fn available_units(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Number of requests currently waiting for capacity.
    pub fn waiting(&self) -> usize {
        self.waiting.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl Gatekeeper for SemaphoreGatekeeper {
    async fn admit(&self, cost: Cost) -> Result<Permit, AdmissionError> {
        let units = cost.0.max(1); // a zero-cost request still takes one unit
        if units as usize > self.capacity {
            // Could never be satisfied — fail fast rather than wait forever.
            return Err(AdmissionError::CostTooLarge {
                cost: units,
                capacity: self.capacity,
            });
        }

        // Fast path: capacity available right now, no queueing.
        if let Ok(p) = self.semaphore.clone().try_acquire_many_owned(units) {
            return Ok(Permit::with_units(cost, p));
        }

        // Backpressure: reject if the wait queue is already full.
        if self.waiting.load(Ordering::Relaxed) >= self.max_queue_depth {
            return Err(AdmissionError::Rejected);
        }

        // Queue (count ourselves as waiting; the guard decrements on every exit path).
        let _guard = WaitingGuard::enter(&self.waiting);
        let acquired = tokio::time::timeout(
            self.acquire_timeout,
            self.semaphore.clone().acquire_many_owned(units),
        )
        .await;

        match acquired {
            Ok(Ok(p)) => Ok(Permit::with_units(cost, p)),
            Ok(Err(_)) => Err(AdmissionError::Rejected), // semaphore closed
            Err(_) => Err(AdmissionError::Timeout),
        }
    }
}

/// RAII counter guard: increments the waiting count on enter, decrements on drop (so every
/// exit path — success, timeout, cancellation — is accounted for).
struct WaitingGuard<'a>(&'a AtomicUsize);

impl<'a> WaitingGuard<'a> {
    fn enter(counter: &'a AtomicUsize) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self(counter)
    }
}

impl Drop for WaitingGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn admits_within_capacity() {
        let gk = SemaphoreGatekeeper::new(4, 8, Duration::from_millis(50));
        let p = gk.admit(Cost(2)).await.unwrap();
        assert_eq!(gk.available_units(), 2);
        drop(p);
        assert_eq!(
            gk.available_units(),
            4,
            "dropping the permit releases units"
        );
    }

    #[tokio::test]
    async fn rejects_cost_larger_than_capacity() {
        let gk = SemaphoreGatekeeper::new(2, 8, Duration::from_millis(50));
        assert!(matches!(
            gk.admit(Cost(3)).await,
            Err(AdmissionError::CostTooLarge { .. })
        ));
    }

    #[tokio::test]
    async fn times_out_when_capacity_held() {
        let gk = SemaphoreGatekeeper::new(1, 8, Duration::from_millis(30));
        let _held = gk.admit(Cost(1)).await.unwrap(); // exhausts capacity
        let start = std::time::Instant::now();
        let r = gk.admit(Cost(1)).await;
        assert!(matches!(r, Err(AdmissionError::Timeout)));
        assert!(start.elapsed() >= Duration::from_millis(25));
    }

    #[tokio::test]
    async fn rejects_when_queue_full() {
        // capacity 1, queue depth 0 → no waiting allowed → immediate reject when full.
        let gk = SemaphoreGatekeeper::new(1, 0, Duration::from_millis(50));
        let _held = gk.admit(Cost(1)).await.unwrap();
        assert!(matches!(
            gk.admit(Cost(1)).await,
            Err(AdmissionError::Rejected)
        ));
    }

    #[tokio::test]
    async fn releases_let_a_waiter_through() {
        let gk = SemaphoreGatekeeper::new(1, 4, Duration::from_secs(2));
        let held = gk.admit(Cost(1)).await.unwrap();
        let gk2 = gk.clone();
        let waiter = tokio::spawn(async move { gk2.admit(Cost(1)).await });
        // Give the waiter a moment to start queueing, then release.
        tokio::time::sleep(Duration::from_millis(20)).await;
        drop(held);
        assert!(
            waiter.await.unwrap().is_ok(),
            "released capacity should admit the waiter"
        );
    }
}
