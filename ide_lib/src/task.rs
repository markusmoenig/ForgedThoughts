use crate::prelude::*;
use forgedthoughts::prelude::*;

use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;

#[derive(Debug)]
pub struct Task {
    pub u8_buffer                   : Vec<u8>,
    pub u8_width                    : usize,
    pub u8_height                   : usize,
    pub u8_receiver                 : Option<Receiver<RenderProgress>>,
}

impl Task {

    pub fn new(x: usize, y: usize, width: usize, height: usize) -> Self {


    }
}