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
//! | Settings (the dialog and the tab) | Paramètres |
//! | Instance defaults | Paramètres par défaut des instances |
//! | shared game settings | paramètres de jeu partagés |
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
//! | username / password | nom d’utilisateur / mot de passe |
//! | user / owner / permission | utilisateur / propriétaire / autorisation |
//! | API key / token | clé d’API / jeton |
//! | start / stop / restart / kill | démarrer / arrêter / redémarrer / forcer l’arrêt |
//! | running / stopped | en marche / arrêté |
//! | port / address / connection | port / adresse / connexion |
//! | enable / disable | activer / désactiver |
//! | the Trash | la Corbeille |
//! | Keychain | trousseau |
//!
//! The words match the Lodestone dashboard's (`dashboard/src/i18n/fr.ts`).
//!
//! Style: sentence case, « vous », infinitives on buttons (« Enregistrer »), the
//! typographic apostrophe (’), and French spacing: a no-break space (U+00A0)
//! before « : », inside « », in grouped numbers and before units ("4 Go"), and a
//! narrow no-break space (U+202F) before ; ! ?. Product names stay in English.

pub mod common;
pub mod content;
pub mod format;
pub mod instance_settings;
pub mod instances;
pub mod servers;
pub mod shell;
pub mod worlds;
