// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { GameType } from "./GameType";
import type { MinecraftVariant } from "./MinecraftVariant";

export type Game = { "type": "MinecraftJava", variant: MinecraftVariant, } | { "type": "MinecraftBedrock" } | { "type": "Generic", game_name: GameType, game_display_name: string, };