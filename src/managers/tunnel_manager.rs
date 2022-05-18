use rand::{distributions::Alphanumeric, Rng};
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
};
pub struct TunnelManager {
    path_to_exe: PathBuf,
    auth_key: String,
    local_port: u32,
    domain: String,
    process: Option<Child>,
}

pub enum TunnelErr {
    FailedToStart,
    DomainTaken,
    Timeout,
}

impl TunnelManager {
    pub fn new(
        path_to_exe: PathBuf,
        auth_key: String,
        local_port: u32,
        domain: Option<String>,
    ) -> TunnelManager {
        // randomly generate a domain name if none is provided
        let domain = match domain {
            Some(domain) => domain,
            None => {
                let mut rng = rand::thread_rng();
                let domain = format!(
                    "{}-{}",
                    "temp",
                    rng.sample_iter(&Alphanumeric).take(5).collect::<String>().to_ascii_lowercase()
                );
                domain
            }
        };
        TunnelManager {
            path_to_exe,
            local_port,
            auth_key,
            domain,
            process: None,
        }
    }

    pub fn start(&mut self) -> Result<(), TunnelErr> {
        let mut cmd = Command::new(&self.path_to_exe);
        info!("attemping to start with domain: {}", &self.domain);
        cmd.arg("http")
            .arg("-s")
            .arg("ca-1.lodestone.link:7000")
            .arg("-t")
            .arg(&self.auth_key)
            .arg("--sd")
            .arg(&self.domain)
            .arg("-l")
            .arg(self.local_port.to_string())
            .arg("-n")
            .arg(&self.domain)
            .stdout(Stdio::piped());
        if let Ok(mut proc) = cmd.spawn() {
            let reader = BufReader::new(proc.stdout.take().unwrap());
            for (pos, line_result) in reader.lines().enumerate() {
                let line = line_result.unwrap();
                if pos > 10 {
                    return Err(TunnelErr::Timeout);
                }
                if line.contains("start proxy success") {
                    self.process = Some(proc);
                    return Ok(());
                } else if line.contains("start error") {
                    proc.kill();
                    return Err(TunnelErr::DomainTaken);
                }
            }
        }
        Err(TunnelErr::FailedToStart)
    }
}
