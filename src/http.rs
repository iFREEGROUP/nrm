use once_cell::sync::Lazy;
use std::future::Future;
use tokio::sync::Semaphore;

static THROTTLE: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(50));

pub(crate) async fn retry<O, R, F>(times: usize, mut f: F) -> reqwest::Result<O>
where
    R: Future<Output = reqwest::Result<O>>,
    F: FnMut() -> R,
{
    let mut t = 1;
    let mut result = {
        let _permit = THROTTLE.acquire().await;
        f().await
    };

    while result.is_err() && t < times {
        let _permit = THROTTLE.acquire().await;
        result = f().await;
        t += 1;
    }

    result
}
