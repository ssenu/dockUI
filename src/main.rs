#![cfg_attr(not(test), windows_subsystem = "windows")]
//! YT Launcher 진입점.
// 크롬 실행·항목 모델은 2단계(화면)에서 연결된다. 그때 이 줄을 지운다.
#![allow(dead_code)]

mod autostart;
mod config;
mod items;
mod updater;
mod win;

fn main() {}
