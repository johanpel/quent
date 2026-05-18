use quent_v2_model::Fsm;

#[derive(Fsm)]
#[quent(transitions(
    entry -> A,
    A -> exit
))]
enum OneUnit {
    A,
}

#[derive(Fsm)]
#[quent(transitions(
    entry -> A,
    A -> B,
    B -> exit
))]
enum TwoUnit {
    A,
    B,
}
