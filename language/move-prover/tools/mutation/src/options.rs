// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use structopt::StructOpt;
use std::collections::BTreeMap;
use std::error::Error;
/// Options passed into the specification flattening tool.

fn parse_key_val<T, U>(s: &str) -> Result<(T, U), Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + 'static,
    U: std::str::FromStr,
    U::Err: Error + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`", s))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}
#[derive(StructOpt, Clone,Debug)]
pub struct MutationOptions {

    /// Sources of the target modules
    pub srcs: Vec<String>,

    /// Dependencies
    #[structopt(short = "d", long = "dependency")]
    pub deps: Vec<String>,

    /// Do not include default named address
    #[structopt(long = "no-default-named-addresses")]
    pub no_default_named_addresses: bool,

    /// Target function
    #[structopt(short, long)]
    pub target: Option<String>,

    /// select mutation type, might be helpful for the future implementation
    #[structopt(short = "D", parse(try_from_str = parse_key_val), number_of_values = 1)]
    pub mutation_option: Vec<(String, bool)>,

}
