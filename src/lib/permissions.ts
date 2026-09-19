import { createContext } from "react";
import type { SessionUser } from "./types";
export const RoleContext = createContext<SessionUser["role"]>("recepcion");
export const ADMIN_COMMANDS = new Set(["save_room", "save_rate_plan", "save_product", "set_product_active", "add_charge", "save_settings", "print_test", "list_printers", "auth_create_user"]);
export const ADMIN_ROUTES = new Set(["/habitaciones", "/catalogo", "/ajustes", "/usuarios"]);
