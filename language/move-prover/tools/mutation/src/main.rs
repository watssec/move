// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use structopt::StructOpt;
use mutation::run;
use mutation::MutationOptions;

fn main() -> Result<()> {
    let mut options = MutationOptions::from_args();
    run(&mut options)?;
    Ok(())
}

