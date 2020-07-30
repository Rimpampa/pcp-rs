use super::event::Event;
use super::map::{InboundMap, Map, OutboundMap};
use super::state::{AtomicState, MapHandle, State};
use super::IpAddress;
use crate::types::ParsingError;
use std::io;
use std::sync::mpsc::{self, RecvError};
use std::sync::Arc;

/// Error generated by PCP operations
#[derive(Debug)]
pub enum Error {
    Socket(io::Error),
    Channel(RecvError),
    Parsing(ParsingError),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Socket(err)
    }
}

impl From<RecvError> for Error {
    fn from(err: RecvError) -> Self {
        Self::Channel(err)
    }
}

impl From<ParsingError> for Error {
    fn from(err: ParsingError) -> Self {
        Self::Parsing(err)
    }
}

/// An handle to a PCP client
pub struct Handle<Ip: IpAddress> {
    to_client: mpsc::Sender<Event<Ip>>,
    from_client: mpsc::Receiver<Error>,
}

impl<Ip: IpAddress> Handle<Ip> {
    pub(crate) fn new(
        to_client: mpsc::Sender<Event<Ip>>,
        from_client: mpsc::Receiver<Error>,
    ) -> Self {
        Handle {
            to_client,
            from_client,
        }
    }
    /// Waits for an error to arrive
    pub fn wait_err(&self) -> Error {
        self.from_client.recv().unwrap_or_else(Error::from)
    }
    /// Returns `Some(Error)` if an error has been received, `None` otherwise
    pub fn poll_err(&self) -> Option<Error> {
        self.from_client.try_recv().ok()
    }
    /// Signal to the client thread to stop
    pub fn shutdown(self) {
        self.to_client.send(Event::Shutdown).ok();
    }
}

#[derive(Debug, PartialEq)]
pub enum RequestType {
    Once,
    Repeat(usize),
    KeepAlive,
}

pub trait Request<Ip: IpAddress, M: Map<Ip>> {
    fn request(&self, map: M, kind: RequestType) -> Result<MapHandle<Ip>, Error>;
}

impl<Ip: IpAddress> Request<Ip, InboundMap<Ip>> for Handle<Ip> {
    fn request(&self, map: InboundMap<Ip>, kind: RequestType) -> Result<MapHandle<Ip>, Error> {
        let (id_tx, id_rx) = mpsc::channel();
        let (alert_tx, alert_rx) = mpsc::channel();
        let state = Arc::new(AtomicState::new(State::Requested));
        self.to_client
            .send(Event::InboundMap(
                map,
                kind,
                Arc::clone(&state),
                id_tx,
                alert_tx,
            ))
            .unwrap();
        if let Some(id) = id_rx.recv().unwrap() {
            Ok(MapHandle::new(id, state, self.to_client.clone(), alert_rx))
        } else {
            Err(self.wait_err())
        }
    }
}

impl<Ip: IpAddress> Request<Ip, OutboundMap<Ip>> for Handle<Ip> {
    fn request(&self, map: OutboundMap<Ip>, kind: RequestType) -> Result<MapHandle<Ip>, Error> {
        let (id_tx, id_rx) = mpsc::channel();
        let (alert_tx, alert_rx) = mpsc::channel();
        let state = Arc::new(AtomicState::new(State::Requested));
        self.to_client
            .send(Event::OutboundMap(
                map,
                kind,
                Arc::clone(&state),
                id_tx,
                alert_tx,
            ))
            .unwrap();
        if let Some(id) = id_rx.recv().unwrap() {
            Ok(MapHandle::new(id, state, self.to_client.clone(), alert_rx))
        } else {
            Err(self.wait_err())
        }
    }
}

impl<Ip: IpAddress> Drop for Handle<Ip> {
    fn drop(&mut self) {
        self.to_client.send(Event::Shutdown).ok();
    }
}
