//  LIB.rs
//    by Lut99
//
//  Created:
//    11 Oct 2024, 16:35:23
//  Last edited:
//    06 May 2025, 12:50:19
//  Auto updated?
//    Yes
//
//  Description:
//
//! [Workflow]: ::workflow::Workflow
#![doc = include_str!("../README.md")]

// Declare the modules
pub mod config;
mod reasonerconn;
mod workflow;

// Use some of it
pub use reasonerconn::*;
