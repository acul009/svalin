//! Heavily WIP
//!
//! This module contains the asynchronous communication logic for the Svalin library.
//! The current code is mostly for experimentation and figuring out the correct API design.

use svalin_pki::{Certificate, mls::SvalinProvider};
use svalin_sysctl::sytem_report::SystemReport;

mod client;
mod agent;

