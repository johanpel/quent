pub enum State {
    Entry,
    Exit,
    State(String),
}

pub struct Transition {
    pub source: State,
    pub target: State,
}

pub struct Fsm {
    pub transitions: Vec<Transition>,
}
