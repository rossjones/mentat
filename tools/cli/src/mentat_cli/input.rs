// Copyright 2017 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use std::io::stdin;

use linefeed::Reader;
use linefeed::terminal::DefaultTerminal;

use std::io::BufReader;
use std::io::BufRead;
use std::fs::File;
use std::path::Path;

use self::InputResult::*;

use edn;

use command_parser::{
    Command, 
    command
};

use errors as cli;

/// Starting prompt
const DEFAULT_PROMPT: &'static str = "mentat=> ";
/// Prompt when further input is being read
// TODO: Should this actually reflect the current open brace?
const MORE_PROMPT: &'static str = "mentat.> ";

/// Possible results from reading input from `InputReader`
#[derive(Clone, Debug)]
pub enum InputResult {
    /// mentat command as input; (name, rest of line)
    MetaCommand(Command),
    /// An empty line
    Empty,
    /// Needs more input
    More,
    /// End of file reached
    Eof,
}

/// Reads input from `stdin`
pub struct InputReader {
    buffer: String,
    reader: Option<Reader<DefaultTerminal>>,
    in_process_cmd: Option<Command>,
}

impl InputReader {
    /// Constructs a new `InputReader` reading from `stdin`.
    pub fn new() -> InputReader {
        let r = match Reader::new("mentat") {
            Ok(mut r) => {
                r.set_word_break_chars(" \t\n!\"#$%&'()*+,-./:;<=>?@[\\]^`");
                Some(r)
            }
            Err(_) => None
        };

        InputReader{
            buffer: String::new(),
            reader: r,
            in_process_cmd: None,
        }
    }

    /// Returns whether the `InputReader` is reading from a TTY.
    pub fn is_tty(&self) -> bool {
        self.reader.is_some()
    }

    pub fn read_file(&mut self, filename: &String) -> Result<Vec<InputResult>, cli::Error> {
        let path = Path::new(&filename);
        let display = path.display();

        let f = match File::open(&path) {
            Err(err) => bail!(cli::ErrorKind::FileError(display.to_string(), err.to_string())),
            Ok(file) => file,
        };

        let mut buffer = String::new();

        let file = BufReader::new(&f);
        let mut cmd_err: Option<edn::ParseError> = None;

        let mut results: Vec<InputResult> = Vec::new();
        for line in file.lines() {
            let l = line.unwrap();
            println!("{}", l);
            buffer.push_str(&l);
            let cmd = Command::Transact(buffer.to_string());
            let (is_complete, err) = cmd.is_complete();
            if is_complete {
                results.push(InputResult::MetaCommand(cmd));
                buffer.clear();
                cmd_err = None;
            } else {
                cmd_err = err;
            }

        }

        if let Some(err) = cmd_err {
            bail!(err);
        }

        Ok(results)
    }

    /// Reads a single command, item, or statement from `stdin`.
    /// Returns `More` if further input is required for a complete result.
    /// In this case, the input received so far is buffered internally.
    pub fn read_input(&mut self) -> Result<InputResult, cli::Error> {
        let prompt = if self.in_process_cmd.is_some() { MORE_PROMPT } else { DEFAULT_PROMPT };
        let line = match self.read_line(prompt) {
            Some(s) => s,
            None => return Ok(Eof),
        };

        self.buffer.push_str(&line);

        if self.buffer.is_empty() {
            return Ok(Empty);
        }

        self.add_history(&line);

        // if we have a command in process (i.e. in incomplete query or transaction),
        // then we already know which type of command it is and so we don't need to parse the
        // command again, only the content, which we do later.
        // Therefore, we add the newly read in line to the existing command args.
        // If there is no in process command, we parse the read in line as a new command.
        let cmd = match &self.in_process_cmd {
            &Some(Command::Query(ref args)) => {
                Command::Query(args.clone() + " " + &line)
            },
            &Some(Command::Transact(ref args)) => {
                Command::Transact(args.clone() + " " + &line)
            },
            _ => {
                let res = command(&self.buffer);
                match res {
                    Ok(cmd) => cmd,
                    Err(err) => {
                        self.buffer.clear();
                        bail!(err)
                    }
                }
            }
        };

        let (is_complete, _) = cmd.is_complete();
        if is_complete {
            self.buffer.clear();
            self.in_process_cmd = None;
            Ok(InputResult::MetaCommand(cmd))
        } else {
            // a query or transact is complete if it contains a valid edn.
            // if the command is not complete, ask for more from the repl and remember
            // which type of command we've found here.
            self.in_process_cmd = Some(cmd);
            Ok(More)
        }
    }

    fn read_line(&mut self, prompt: &str) -> Option<String> {
        match self.reader {
            Some(ref mut r) => {
                r.set_prompt(prompt);
                r.read_line().ok().and_then(|line| line)
            },
            None => self.read_stdin()
        }
    }

    fn read_stdin(&self) -> Option<String> {
        let mut s = String::new();

        match stdin().read_line(&mut s) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(s)
        }
    }

    fn add_history(&mut self, line: &str) {
        if let Some(ref mut r) = self.reader {
            r.add_history(line.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
