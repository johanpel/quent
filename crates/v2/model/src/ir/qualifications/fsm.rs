/// IR of an FSM state
#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Entry,
    Exit,
    State(String),
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
