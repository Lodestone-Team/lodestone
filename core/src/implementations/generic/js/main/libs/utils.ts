import { ProcedureCallInner } from "./bindings/ProcedureCallInner.ts";

export function isTServer(inner: ProcedureCallInner): boolean {
    switch (inner.type) {
      case "StartInstance":
      case "StopInstance":
      case "RestartInstance":
      case "KillInstance":
      case "SendCommand":
      case "GetState":
      case "Monitor":
        return true;
    }
    return false;
  }
  
  export function isTPlayer(inner: ProcedureCallInner): boolean {
    switch (inner.type) {
      case "GetPlayerCount":
      case "GetPlayerList":
      case "GetMaxPlayerCount":
        return true;
    }
    return false;
  }
  
  export function isTConfig(inner: ProcedureCallInner): boolean {
    switch (inner.type) {
      case "GetName":
      case "GetDescription":
      case "GetVersion":
      case "GetGame":
      case "GetPort":
      case "GetAutoStart":
      case "GetRestartOnCrash":
      case "GetConfigurableManifest":
      case "SetName":
      case "SetDescription":
      case "SetPort":
      case "SetAutoStart":
      case "SetRestartOnCrash":
      case "UpdateConfigurable":
        return true;
    }
    return false;
  }
  
  export function isTMacro(inner: ProcedureCallInner): boolean {
    switch (inner.type) {
      case "GetMacroList":
      case "GetTaskList":
      case "GetHistoryList":
      case "DeleteMacro":
      case "CreateMacro":
      case "RunMacro":
        return true;
    }
    return false;
  }