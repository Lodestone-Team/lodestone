![workflow](https://github.com/LodestoneMC-org/backend/actions/workflows/backend.yml/badge.svg)

# Lodestone

## What is Lodestone?
Lodestone is a wrapper tool that aims to provide an easy and consistent server hosting experience for Minecraft.

**Lodestone is still in the very early stage of development, as such you should use it ONLY if you know what you are doing**

---

## Setup
### Ubuntu:

The following assumes that you are already in the `Lodestone` directory.

Run the ```dev_setup.sh``` script, this will install all the dependencies.

To run the front end, `cd` into `frontend`. Run `npm i` to install packages then `npm start` to start the dev server.

To run the back end, 
`cd` into `backend`, make a directory for the program files with `mkdir LodestoneTest`. `cd` into `LodestoneTest` and set it as the home directory with `export LODESTONE_PATH=$PWD`.

On a *separate terminal*, make a directory for the database with `mkdir db`, then run `mongod --dbpath db`.

Finally, `cd` back to `backend` and run `cargo run`

### Windows:
Windows support is planned.

---

[Trello](https://trello.com/b/sCaSEPyU/lodestone)
[Figma](https://www.figma.com/file/gM7KUynANg4JkGF3QBsYJ9/Lodestone?node-id=166%3A1621)
