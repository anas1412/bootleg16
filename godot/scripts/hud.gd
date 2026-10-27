## The P0 readout: your speed (units a second, as CS shows it) to judge bunny hops, and the
## frame rate.
extends Label

var session: Session

func _process(_delta: float) -> void:
	if session == null:
		return
	text = "%d u/s    %d fps" % [round(session.speed()), Engine.get_frames_per_second()]
