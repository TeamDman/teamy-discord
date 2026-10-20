use super::MAX_FRAME_BYTES;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
pub(super) async fn write<T: facet::Facet<'static>>(
    stream: &mut (impl AsyncWrite + Unpin),
    value: &T,
) -> eyre::Result<()> {
    let bytes = facet_json::to_string(value)?.into_bytes();
    eyre::ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_FRAME_BYTES,
        "Watcher IPC frame too large"
    );
    stream.write_u32_le(u32::try_from(bytes.len())?).await?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    Ok(())
}
pub(super) async fn read<T: for<'a> facet::Facet<'a>>(
    stream: &mut (impl AsyncRead + Unpin),
) -> eyre::Result<T> {
    let length = stream.read_u32_le().await?;
    eyre::ensure!(
        length > 0 && length as usize <= MAX_FRAME_BYTES,
        "Invalid watcher IPC frame size"
    );
    let mut bytes = vec![0; length as usize];
    stream.read_exact(&mut bytes).await?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_invalid_utf8| eyre::eyre!("Invalid watcher IPC encoding"))?;
    facet_json::from_str(text).map_err(|_invalid_schema| eyre::eyre!("Invalid watcher IPC schema"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn bounded_frames_roundtrip_and_reject_oversized_or_empty_input() {
        let (mut client, mut server) = tokio::io::duplex(256);
        let writer = tokio::spawn(async move {
            write(&mut client, &"fixture".to_owned()).await.unwrap();
        });
        let result: String = read(&mut server).await.unwrap();
        assert_eq!(result, "fixture");
        writer.await.unwrap();
        for length in [0, u32::try_from(MAX_FRAME_BYTES + 1).unwrap()] {
            let (mut client, mut server) = tokio::io::duplex(256);
            client.write_u32_le(length).await.unwrap();
            let _error = read::<String>(&mut server).await.unwrap_err();
        }
    }
}
