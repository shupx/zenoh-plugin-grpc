use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use flume::{r#async::RecvFut, Receiver, RecvError, Sender, TryRecvError, TrySendError};
use tokio::sync::Notify;

#[derive(Clone)]
pub struct DropOldestSender<T> {
    tx: Sender<T>,
    drop_rx: Receiver<T>,
    dropped: Arc<AtomicU64>,
    pending: Arc<AtomicU64>,
    notify: Arc<Notify>,
}

#[derive(Clone)]
pub struct DropOldestReceiver<T> {
    rx: Receiver<T>,
    dropped: Arc<AtomicU64>,
    pending: Arc<AtomicU64>,
    notify: Arc<Notify>,
}

pub fn bounded_drop_oldest<T>(capacity: usize) -> (DropOldestSender<T>, DropOldestReceiver<T>) {
    let (tx, rx) = flume::bounded(capacity);
    let dropped = Arc::new(AtomicU64::new(0));
    let pending = Arc::new(AtomicU64::new(0));
    let notify = Arc::new(Notify::new());
    (
        DropOldestSender {
            tx: tx.clone(),
            drop_rx: rx.clone(),
            dropped: dropped.clone(),
            pending: pending.clone(),
            notify: notify.clone(),
        },
        DropOldestReceiver {
            rx,
            dropped,
            pending,
            notify,
        },
    )
}

impl<T> DropOldestSender<T> {
    pub fn push(&self, mut item: T) -> Result<(), T> {
        loop {
            match self.tx.try_send(item) {
                Ok(()) => {
                    self.pending.fetch_add(1, Ordering::Relaxed);
                    return Ok(());
                }
                Err(TrySendError::Disconnected(item)) => return Err(item),
                Err(TrySendError::Full(full_item)) => {
                    item = full_item;
                    match self.drop_rx.try_recv() {
                        Ok(_) => {
                            self.dropped.fetch_add(1, Ordering::Relaxed);
                            self.pending.fetch_sub(1, Ordering::Relaxed);
                            self.notify.notify_waiters();
                        }
                        Err(TryRecvError::Disconnected) => return Err(item),
                        Err(TryRecvError::Empty) => {}
                    }
                }
            }
        }
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub async fn wait_empty(&self) {
        while self.pending.load(Ordering::Relaxed) != 0 {
            self.notify.notified().await;
        }
    }
}

impl<T> DropOldestReceiver<T> {
    pub fn recv(&self) -> Result<T, RecvError> {
        self.rx.recv()
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.rx.try_recv()
    }

    pub fn recv_async(&self) -> RecvFut<'_, T> {
        self.rx.recv_async()
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn is_disconnected(&self) -> bool {
        self.rx.is_disconnected()
    }

    pub fn processed_one(&self) {
        self.pending.fetch_sub(1, Ordering::Relaxed);
        self.notify.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::bounded_drop_oldest;

    #[test]
    fn drops_oldest_when_full() {
        let (tx, rx) = bounded_drop_oldest(2);
        tx.push(1).unwrap();
        tx.push(2).unwrap();
        tx.push(3).unwrap();

        assert_eq!(tx.dropped_count(), 1);
        assert_eq!(rx.dropped_count(), 1);
        assert_eq!(rx.recv().unwrap(), 2);
        assert_eq!(rx.recv().unwrap(), 3);
    }
}
