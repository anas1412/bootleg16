## Benchmark mode (BOOTLEG16_BENCH=1): stands still at the spawn, renders a few seconds under
## each setting with vsync off, prints frames a second with where the time went (CPU frame,
## GPU frame, simulation ticks), then quits. To find what costs frames.
extends Node

const SECONDS := 3.0

var _cases: Array = []
var _case := -1
var _t := 0.0
var _frames := 0
var _gpu := 0.0
var _cpu := 0.0
var _rows: PackedStringArray = []

func _ready() -> void:
	DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
	var vp := get_viewport()
	RenderingServer.viewport_set_measure_render_time(vp.get_viewport_rid(), true)
	var sun: DirectionalLight3D = get_parent().get_node("Sun")
	var env: Environment = get_parent().get_node("Environment").environment
	_cases = [
		["as is", func(): pass],
		["no shadows", func(): sun.shadow_enabled = false],
		["no shadows, no MSAA", func(): vp.msaa_3d = Viewport.MSAA_DISABLED],
		["+ no sky reflections", func(): env.reflected_light_source = Environment.REFLECTION_SOURCE_DISABLED],
		["shadows, SMAA instead of MSAA", func():
			sun.shadow_enabled = true
			env.reflected_light_source = Environment.REFLECTION_SOURCE_BG
			vp.screen_space_aa = Viewport.SCREEN_SPACE_AA_SMAA],
		["same, 3D at half resolution", func(): vp.scaling_3d_scale = 0.5],
	]
	_next()

func _next() -> void:
	if _case >= 0:
		var n: float = max(_frames, 1)
		_rows.append("%-28s %5d fps   cpu %5.2f ms   gpu %5.2f ms   sim %5.2f ms" % [_cases[_case][0], round(_frames / SECONDS), _cpu / n, _gpu / n, Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS) * 1000.0])
	_case += 1
	if _case >= _cases.size():
		print("\nbootleg16 benchmark (1 s warm-up, %d s each):" % SECONDS)
		for r in _rows:
			print(r)
		get_tree().quit()
		return
	_cases[_case][1].call()
	_t = -1.0
	_frames = 0
	_gpu = 0.0
	_cpu = 0.0

func _process(delta: float) -> void:
	_t += delta
	if _t < 0.0:
		return
	var rid := get_viewport().get_viewport_rid()
	_frames += 1
	_gpu += RenderingServer.viewport_get_measured_render_time_gpu(rid)
	_cpu += RenderingServer.viewport_get_measured_render_time_cpu(rid) + RenderingServer.get_frame_setup_time_cpu()
	if _t >= SECONDS:
		_next()
