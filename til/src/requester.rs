use crossbeam::channel::Receiver;

pub trait Requester<Request>: Send {
    fn run(&mut self, request_rx: Receiver<Request>);
}
