## Weapons as players read them: CS's names (what the simulation calls them) to the names
## the HUD, the kill feed and the buy menu show.
class_name Weapons
extends RefCounted

const NAMES := {
	"knife": "Knife", "glock18": "Glock-18", "usp": "USP", "p228": "P228", "deagle": "Desert Eagle",
	"mac10": "MAC-10", "tmp": "TMP", "mp5navy": "MP5", "ump45": "UMP45", "p90": "P90",
	"galil": "Galil", "famas": "FAMAS", "ak47": "AK-47", "m4a1": "M4A1", "scout": "Scout", "awp": "AWP",
	"hegrenade": "HE Grenade", "flashbang": "Flashbang", "smokegrenade": "Smoke Grenade", "c4": "Bomb",
	"bomb": "Bomb", "": "Fall",
}

static func shown(cs_name: String) -> String:
	return NAMES.get(cs_name, cs_name)

## The HUD icon (`ui/icons/hud/<icon>.png`) for a weapon, as client/src/hud/icons.rs picks
## it; a kill with no weapon (the bomb, a fall) is the skull.
const ICONS := {
	"knife": "d_knife", "glock18": "d_glock18", "usp": "d_usp", "p228": "d_p228", "deagle": "d_deagle",
	"mac10": "d_mac10", "tmp": "d_tmp", "mp5navy": "d_mp5navy", "ump45": "d_ump45", "p90": "d_p90",
	"galil": "d_galil", "famas": "d_famas", "ak47": "d_ak47", "m4a1": "d_m4a1", "scout": "d_scout", "awp": "d_awp",
	"hegrenade": "d_grenade", "flashbang": "d_flashbang", "smokegrenade": "d_smoke", "c4": "c4",
}

static func icon(cs_name: String) -> String:
	return ICONS.get(cs_name, "d_skull")
