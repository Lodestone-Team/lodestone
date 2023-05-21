// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { UserPermission } from "./UserPermission.ts";

export type UserEventInner = { type: "UserCreated" } | { type: "UserDeleted" } | { type: "UserLoggedIn" } | { type: "UserLoggedOut" } | { type: "UsernameChanged", new_username: string, } | { type: "PermissionChanged", new_permissions: UserPermission, };