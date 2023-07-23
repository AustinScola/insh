use crossbeam::channel::Sender;

pub trait ResponseHandler<Response>: Send {
    fn run(&mut self, response_tx: Sender<Response>);
}
