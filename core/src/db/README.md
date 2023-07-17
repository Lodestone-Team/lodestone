# Database

## IDEA
The project is rapidly evolving, thus the current database design philosophy is **functionality over efficiency**
We attempted to use document database but dropped it due to size and complexity setting up
Current implementation uses Sqlite, however in a document db fashion

## Notes
The `ClientEvents` table schema is in `migrations` folder, in the future, depending on how often we modify DB, we might implement auto migration or use ORM
