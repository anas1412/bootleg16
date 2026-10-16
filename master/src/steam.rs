//! Checking Steam logins: the lobby signs in to Steam as an anonymous game server and asks
//! Steam whether each player's ticket is real.
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use steamworks::{Server, ServerMode, SteamId, ValidateAuthTicketResponse};

pub struct Checker {
    server: Server,
    answers: Receiver<ValidateAuthTicketResponse>,
    /// Steam id -> the lobby session waiting for Steam's answer.
    waiting: HashMap<u64, u64>,
    _callback: steamworks::CallbackHandle,
}

impl Checker {
    pub fn start(app: u32) -> Checker {
        std::env::set_var("SteamAppId", app.to_string());
        let (server, _) = Server::init([0, 0, 0, 0].into(), 0, 0, ServerMode::Authentication, env!("CARGO_PKG_VERSION")).unwrap_or_else(|e| panic!("Steam: {e} (is Steam's steamclient.so installed? see README)"));
        server.set_dedicated_server(true);
        server.log_on_anonymous();
        let (tx, answers) = channel();
        let callback = server.register_callback(move |r: ValidateAuthTicketResponse| _ = tx.send(r));
        println!("checking Steam logins (app {app})");
        Checker { server, answers, waiting: HashMap::new(), _callback: callback }
    }

    /// Start checks for new logins and hand Steam's answers to the lobby.
    pub fn run(&mut self, m: &mut net::master::Master) {
        for (session, id, ticket) in m.take_steam_checks() {
            let user = SteamId::from_raw(id);
            // A fresh login replaces an older one of the same account.
            self.server.end_authentication_session(user);
            match self.server.begin_authentication_session(user, &ticket) {
                Ok(()) => _ = self.waiting.insert(id, session),
                Err(e) => {
                    println!("Steam login {id} refused: {e}");
                    m.steam_refused(session);
                }
            }
        }
        self.server.run_callbacks();
        while let Ok(r) = self.answers.try_recv() {
            let id = r.steam_id.raw();
            let Some(session) = self.waiting.remove(&id) else { continue };
            match r.response {
                Ok(()) => m.steam_checked(id, Some(session)),
                Err(e) => {
                    println!("Steam login {id} refused: {e}");
                    m.steam_refused(session);
                }
            }
        }
    }
}
