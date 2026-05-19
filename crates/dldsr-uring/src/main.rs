use anyhow::Result;

fn main() -> Result<()> {
    println!("dldsr-uring prototype");
    println!("main daemon uses Tokio UDP; future work is batched sendmsg/recvmsg with io_uring SQEs/CQEs");
    Ok(())
}
