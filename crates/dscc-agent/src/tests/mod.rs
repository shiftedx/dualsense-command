#![allow(unused_imports)]

use super::*;
use crate::game_modules::{FORZA_HORIZON5_STEAM_APP_ID, FORZA_HORIZON6_STEAM_APP_ID};
use axum::{
    body::{to_bytes, Body},
    http::{Method, Request},
};
use tower::ServiceExt;

mod support;

mod api;
mod controllers;
mod edge_profiles;
mod effects;
mod forza_glyphs;
mod game_detection;
mod input_bridge;
mod profiles;
mod steam_input;
mod telemetry;
mod user_games;
mod web_dist;
