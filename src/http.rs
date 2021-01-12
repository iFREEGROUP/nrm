use std::future::Future;

pub(crate) async fn retry<O, R, F>(times: usize, mut f: F) -> reqwest::Result<O>
where
    R: Future<Output = reqwest::Result<O>>,
    F: FnMut() -> R,
{
    let mut t = 1;
    let mut result = f().await;

    while result.is_err() && t < times {
        result = f().await;
        t += 1;
    }

    result
}
