use crate::validator::ValidationError;

/// IR of an FSM state
#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Entry,
    Exit,
    State(String), // TODO: maybe add some sort of ANSI-C com
}

impl TryFrom<&str> for State {
    type Error = ValidationError;
    fn try_from(name: &str) -> Result<Self, Self::Error> {
        Ok(if name.eq_ignore_ascii_case("entry") {
            Self::Entry
        } else if name.eq_ignore_ascii_case("exit") {
            Self::Exit
        } else {
            Self::State(name.into())
        })
    }
}
/// IR of an FSM transition
#[derive(Debug, PartialEq, Eq)]
pub struct Transition {
    pub source: State,
    pub target: State,
}

///  IR of an FSM
#[derive(Debug, PartialEq, Eq)]
pub struct Fsm {
    pub transitions: Vec<Transition>,
}
