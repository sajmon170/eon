use futures::prelude::*;
use std::io::prelude::*;

use crate::{
    app::{controller::*, repl::*},
    net::{
        event_loop::{self, Event},
        network::{Client, EventStream},
    },
    CliArgument,
};

use std::error::Error;
use anyhow::Result;

pub struct AppCli {
    controller: AppControllerHandle,
}

impl AppCli {
    pub fn new(network_client: Client, events: EventStream) -> Self {
        Self {
            controller: AppControllerHandle::new(network_client, events),
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let mut is_running = true;
        let mut line = String::new();

        while is_running {
            print!("?> ");
            std::io::stdout().flush().unwrap();
            std::io::stdin().read_line(&mut line)?;
            let args = shlex::split(&line).unwrap();
            let cli = Cli::try_parse_from(args).unwrap();

            self.controller.send(cli.command).await;
        }

        Ok(())
    }
}
