use term::TermEvent;

pub enum Event<Response> {
    TermEvent(TermEvent),
    Response(Response),
}
