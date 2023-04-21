pub mod vm_handler;

use cidr::IpInet;
use log::debug;
use lumper::VMM;
use shared::RequestMessage;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::rc::Rc;
use std::thread::{spawn, JoinHandle};
use std::{os::unix::net::UnixListener, u32};

use crate::net;

#[derive(Debug)]
pub enum Error {
    VmmNew(lumper::Error),

    VmmConfigure(lumper::Error),

    VmmRun(lumper::Error),

    BadAgentStatus,

    NoIPAvalaible,
}

#[derive(Debug, Clone)]
pub struct VMMOpts {
    /// Linux kernel path
    pub kernel: String,
    /// Number of virtual CPUs assigned to the guest
    pub cpus: u8,
    /// Memory amount (in MBytes) assigned to the guest
    pub memory: u32,
    /// Stdout console file path
    pub console: Option<String>,
    /// Path to the socket used for communication with the VMM
    pub socket: Option<String>,
    /// initramfs path
    pub initramfs: Option<String>,
    // Tap interface name
    pub tap: Option<String>,
    // IP address
    pub ip: Option<IpInet>,
    // Gateway
    pub gateway: Option<String>,
}

pub fn run(opts: VMMOpts) -> Result<(), Error> {
    let mut vmm = VMM::new().map_err(Error::VmmNew)?;
    let tap_name = opts.tap.clone();
    vmm.configure(
        opts.cpus,
        opts.memory,
        &opts.kernel,
        opts.console,
        opts.initramfs,
        tap_name,
        opts.socket,
        true,
        Some(opts.ip.unwrap().to_string()),
        opts.gateway,
    )
    .map_err(Error::VmmConfigure)?;

    debug!("Adding interface to bridge");
    net::add_interface_to_bridge(&opts.tap.unwrap(), "lambdo0").unwrap();
    // Run the VMM
    vmm.run(true).map_err(Error::VmmRun)?;

    Ok(())
}

pub fn listen(unix_listener: UnixListener, request_message: RequestMessage) -> JoinHandle<String> {
    spawn(move || {
        // read from socket
        let (mut stream, _) = unix_listener.accept().unwrap();
        let mut response = "".to_string();

        let stream_reader = BufReader::new(stream.try_clone().unwrap());
        let rc = Rc::new(request_message);

        for line in stream_reader.lines() {
            let parsed_line = parse_response(line.unwrap(), &mut stream, rc.clone()).unwrap();
            if parsed_line.is_empty() {
                continue;
            }

            response = format!("{}{}\n", response, parsed_line);
            log::trace!("response line: {}", response);
        }
        log::debug!("response: {}", response);

        response
    })
}

fn parse_response(
    response: String,
    stream: &mut UnixStream,
    request_message: Rc<RequestMessage>,
) -> Result<String, Error> {
    log::trace!("received response from agent: {}", response);
    if response.contains("\"type\":\"status\"") {
        // match the status code
        let status_code = response
            .split("\"code\":")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap()
            .split('"')
            .nth(1)
            .unwrap();
        log::debug!("received status code from agent: {}", status_code);

        if status_code == "ready" {
            send_instructions(stream, request_message);
            Ok("".to_string())
        } else {
            Err(Error::BadAgentStatus)
        }
    } else {
        Ok(response)
    }
}

fn send_instructions(stream: &mut UnixStream, request_message: Rc<RequestMessage>) {
    let message = format_message(serde_json::to_string(&*request_message).unwrap().as_str());

    log::debug!("sending agent execution json: {}", message);

    // send the agent execution to the socket
    stream.write_all(message.as_bytes()).unwrap();
}

fn format_message(message: &str) -> String {
    let message_size = message.len();
    format!("{:0>8}{}", message_size, message)
}
