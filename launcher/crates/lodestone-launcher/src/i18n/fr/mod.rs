//! French (France), `fr`.
//!
//! Glossary: use these words everywhere, so a thing has one name.
//!
//! | English | Français |
//! |---|---|
//! | instance (a launcher game folder) | instance (f.) |
//! | server (a game server a core runs) | serveur |
//! | core (a Lodestone Core) | core (m.), « ce core » ; Lodestone Core stays as it is |
//! | mod / modpack | mod (m.) / modpack (m.) |
//! | mod loader / loader | chargeur de mods / chargeur |
//! | resource pack / shader pack | pack de ressources / pack de shaders |
//! | world / backup / screenshot | monde / sauvegarde / capture d'écran |
//! | Settings (the dialog and the tab) | Réglages |
//! | Instance defaults | Réglages par défaut des instances |
//! | shared game settings | réglages de jeu partagés |
//! | account / offline account | compte / compte hors ligne |
//! | sign in / sign out | se connecter / se déconnecter |
//! | Play | Jouer |
//! | Save (a change) / Discard | Enregistrer / Abandonner |
//! | download / install / update | télécharger / installer / mettre à jour |
//! | game version | version du jeu |
//! | memory | mémoire |
//! | console / log | console / journal |
//! | key binding | touche |
//! | player / operator / whitelist | joueur / opérateur / liste blanche |
//! | folder / file | dossier / fichier |
//! | the Trash | la Corbeille |
//! | Keychain | trousseau |
//!
//! Style: sentence case, « vous », and French spacing: a no-break space (U+00A0)
//! before « : » and inside « », a narrow no-break space (U+202F) before ; ! ? and
//! in grouped numbers and before units ("4 Go"). Product names stay in English.

pub mod common;
pub mod content;
pub mod format;
pub mod instance_settings;
pub mod instances;
pub mod servers;
pub mod shell;
pub mod worlds;
