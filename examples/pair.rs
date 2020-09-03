use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio_serial::Serial;

#[tokio::main]
async fn main() -> io::Result<()> {
    let (mut req, mut res) = Serial::pair()?;

    let f = tokio::spawn(async move {
        let mut buffer = [0u8; 1];
        res.read_exact(&mut buffer).await?;

        let [command] = buffer;
        assert_eq!(command, 1);

        res.write_all(&buffer).await?;
        res.shutdown().await?;

        std::io::Result::<_>::Ok(())
    });
    // Write something
    req.write_all(&[1u8]).await?;
    // Read the answer
    let mut buffer = [0u8; 1];
    req.read_exact(&mut buffer).await?;

    let [response] = buffer;
    assert_eq!(response, 1);

    // may be a join error, or an IO error from inside the task
    f.await??;

    Ok(())
}
