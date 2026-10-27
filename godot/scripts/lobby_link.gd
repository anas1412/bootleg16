## The lobby connection (autoload `LobbyLink`): your party, the casual queue and custom games.
## Lives as long as the game, so the party survives matches. When a match is found it keeps
## the seat in `pending` and opens the game scene, which joins it.
extends Node

signal state_changed(state: Dictionary)
signal notice(text: String)

var lobby := Lobby.new()
var state := {}
## A match to join: {address, ticket, key}, taken by the game scene.
var pending := {}
## The last thing to tell the player, kept until the menu shows it (a failed join happens
## in the game scene, and the menu shows it on its return).
var unread := ""

func _ready() -> void:
	if not lobby.start(player_name(), _maps()):
		push_warning("bootleg16: playing offline (no lobby connection)")

func _process(_delta: float) -> void:
	lobby.tick()
	var now := lobby.state()
	if now != state:
		state = now
		state_changed.emit(state)
	var said := lobby.take_notice()
	if said != "":
		tell(said)
	# a found match is taken only from the menu: one arriving late must not pull you out of
	# the game you're in (it waits until you're back)
	var scene := get_tree().current_scene
	if scene and scene.name == "MainMenu" and pending.is_empty():
		var found := lobby.take_found()
		if not found.is_empty():
			pending = found
			get_tree().change_scene_to_file("res://main.tscn")

## Something for the player to read in the menu.
func tell(text: String) -> void:
	unread = text
	notice.emit(text)

## Takes the unread message ("" if none).
func take_unread() -> String:
	var t := unread
	unread = ""
	return t

func online() -> bool:
	return lobby.is_online()

## You lead your party (only the leader picks the mode and queues).
func leading() -> bool:
	return not state.is_empty() and state.you == state.leader

## Your name until Steam gives one: the computer's user name.
func player_name() -> String:
	for v in ["USER", "USERNAME"]:
		var n := OS.get_environment(v)
		if n != "":
			return n.capitalize()
	return "Player"

## The maps this game can play (the lobby only books maps everyone in the match has).
func _maps() -> PackedStringArray:
	var maps := PackedStringArray()
	for f in DirAccess.get_files_at("res://maps"):
		if f.ends_with(".bmap"):
			maps.append(f.get_basename())
	return maps
