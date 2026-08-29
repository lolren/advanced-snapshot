/* SPDX-License-Identifier: GPL-3.0-or-later */
/* Submit focus and image-adjustment controls to a libcamera PipeWire node. */

#include <errno.h>
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include <pipewire/pipewire.h>

#include <spa/param/param.h>
#include <spa/param/props.h>
#include <spa/pod/builder.h>
#include <spa/pod/parser.h>
#include <spa/utils/string.h>

#define CROP_KEY "api.libcamera.scaler-crop"
#define CROP_MAXIMUM_KEY "api.libcamera.scaler-crop-maximum"
#define ORIENTATION_KEY "api.libcamera.stream-orientation"
#define AF_STATE_KEY "api.libcamera.af-state"
#define AF_TRIGGER_GENERATION_KEY "api.libcamera.af-trigger-generation"
#define AF_STATE_TRIGGER_GENERATION_KEY \
	"api.libcamera.af-state-trigger-generation"

enum operation {
	OPERATION_FOCUS,
	OPERATION_WAIT_FOCUS,
	OPERATION_RESET,
	OPERATION_ADJUST,
	OPERATION_MANUAL_EXPOSURE,
	OPERATION_AUTO_EXPOSURE,
};

enum stage {
	STAGE_DISCOVER,
	STAGE_ENUM_CONTROLS,
	STAGE_APPLY_CONTROLS,
	STAGE_WAIT_AUTOFOCUS,
};

enum autofocus_state {
	AUTOFOCUS_STATE_UNKNOWN,
	AUTOFOCUS_STATE_IDLE,
	AUTOFOCUS_STATE_SCANNING,
	AUTOFOCUS_STATE_FOCUSED,
	AUTOFOCUS_STATE_FAILED,
};

struct app {
	struct pw_main_loop *loop;
	struct pw_context *context;
	struct pw_core *core;
	struct pw_registry *registry;
	struct pw_node *node;
	struct spa_source *timer;
	struct spa_hook core_listener;
	struct spa_hook registry_listener;
	struct spa_hook node_listener;

	enum operation operation;
	enum stage stage;
	uint64_t target_serial;
	double focus_x;
	double focus_y;
	double focus_size;
	double exposure_value;
	double saturation;
	double contrast;
	double sharpness;
	int32_t exposure_time_us;
	double analogue_gain;

	uint32_t af_mode_id;
	uint32_t af_trigger_id;
	uint32_t af_metering_id;
	uint32_t af_windows_id;
	uint32_t exposure_value_id;
	uint32_t saturation_id;
	uint32_t contrast_id;
	uint32_t sharpness_id;
	uint32_t exposure_time_id;
	uint32_t exposure_time_mode_id;
	uint32_t analogue_gain_id;
	uint32_t analogue_gain_mode_id;
	int crop_x;
	int crop_y;
	unsigned int crop_width;
	unsigned int crop_height;
	unsigned int orientation;
	enum autofocus_state autofocus_state;
	uint64_t af_trigger_generation;
	uint64_t af_state_trigger_generation;
	uint64_t baseline_af_trigger_generation;
	uint64_t expected_af_trigger_generation;
	int sync_seq;
	int result;
	bool controls_requested;
	bool crop_available;
	bool af_state_available;
	bool af_trigger_generation_available;
	bool af_state_trigger_generation_available;
	bool finished;
};

static void finish(struct app *app, int result, const char *message)
{
	if (app->finished)
		return;
	app->finished = true;
	if (message)
		fprintf(stderr, "advanced-snapshot-focus-control: %s\n", message);
	app->result = result;
	pw_main_loop_quit(app->loop);
}

static enum autofocus_state parse_autofocus_state(const char *state)
{
	if (!state)
		return AUTOFOCUS_STATE_UNKNOWN;
	if (spa_streq(state, "idle"))
		return AUTOFOCUS_STATE_IDLE;
	if (spa_streq(state, "scanning"))
		return AUTOFOCUS_STATE_SCANNING;
	if (spa_streq(state, "focused"))
		return AUTOFOCUS_STATE_FOCUSED;
	if (spa_streq(state, "failed"))
		return AUTOFOCUS_STATE_FAILED;
	return AUTOFOCUS_STATE_UNKNOWN;
}

static void evaluate_autofocus_result(struct app *app)
{
	if (app->stage != STAGE_WAIT_AUTOFOCUS || app->finished)
		return;

	if (app->expected_af_trigger_generation == 0 &&
	    app->af_trigger_generation_available &&
	    app->af_trigger_generation > app->baseline_af_trigger_generation)
		app->expected_af_trigger_generation = app->af_trigger_generation;

	if (app->expected_af_trigger_generation == 0 ||
	    !app->af_state_trigger_generation_available ||
	    app->af_state_trigger_generation !=
		app->expected_af_trigger_generation)
		return;

	if (app->autofocus_state == AUTOFOCUS_STATE_SCANNING)
		return;

	if (app->autofocus_state == AUTOFOCUS_STATE_FOCUSED ||
	    app->autofocus_state == AUTOFOCUS_STATE_FAILED) {
		puts(app->autofocus_state == AUTOFOCUS_STATE_FOCUSED
			     ? "focused"
			     : "failed");
		fflush(stdout);
		finish(app, 0, NULL);
	}
}

static void evaluate_wait_result(struct app *app)
{
	if (app->operation != OPERATION_WAIT_FOCUS || app->finished)
		return;

	/* A continuously focusing rear camera is safe to capture once the
	 * libcamera algorithm has published a terminal state. A failed scan is
	 * still a useful terminal result: the lens is no longer moving, and the
	 * caller can make the normal capture rather than hanging indefinitely. */
	if (app->autofocus_state == AUTOFOCUS_STATE_FOCUSED) {
		puts("focused");
		fflush(stdout);
		finish(app, 0, NULL);
	} else if (app->autofocus_state == AUTOFOCUS_STATE_FAILED) {
		puts("failed");
		fflush(stdout);
		finish(app, 0, NULL);
	}
}

static bool parse_uint64(const char *text, uint64_t *value)
{
	char *end = NULL;
	errno = 0;
	unsigned long long parsed = strtoull(text, &end, 10);
	if (errno || end == text || *end != '\0')
		return false;
	*value = parsed;
	return true;
}

static bool parse_double(const char *text, double *value)
{
	char *end = NULL;
	errno = 0;
	double parsed = strtod(text, &end);
	if (errno || end == text || *end != '\0' || !isfinite(parsed))
		return false;
	*value = parsed;
	return true;
}

static bool parse_int32(const char *text, int32_t *value)
{
	char *end = NULL;
	errno = 0;
	long parsed = strtol(text, &end, 10);
	if (errno || end == text || *end != '\0' ||
	    parsed < INT32_MIN || parsed > INT32_MAX)
		return false;
	*value = (int32_t)parsed;
	return true;
}

static bool parse_crop(struct app *app, const char *text)
{
	char extra;
	return sscanf(text, "%d,%d,%u,%u%c", &app->crop_x, &app->crop_y,
		      &app->crop_width, &app->crop_height, &extra) == 4 &&
	       app->crop_width > 0 && app->crop_height > 0;
}

static void orient_point(unsigned int orientation, double *x, double *y)
{
	double input_x = *x;
	double input_y = *y;

	/*
	 * libcamera Orientation values follow EXIF orientation tag values and
	 * describe the transform from the naturally oriented sensor image to the
	 * displayed buffer. Apply the inverse transform to map a display tap back
	 * into sensor coordinates.
	 */
	switch (orientation) {
	case 2: /* Rotate0Mirror */
		*x = 1.0 - input_x;
		*y = input_y;
		break;
	case 3: /* Rotate180 */
		*x = 1.0 - input_x;
		*y = 1.0 - input_y;
		break;
	case 4: /* Rotate180Mirror */
		*x = input_x;
		*y = 1.0 - input_y;
		break;
	case 5: /* Rotate90Mirror */
		*x = input_y;
		*y = input_x;
		break;
	case 6: /* Rotate270 */
		*x = 1.0 - input_y;
		*y = input_x;
		break;
	case 7: /* Rotate270Mirror */
		*x = 1.0 - input_y;
		*y = 1.0 - input_x;
		break;
	case 8: /* Rotate90 */
		*x = input_y;
		*y = 1.0 - input_x;
		break;
	case 1: /* Rotate0 */
	default:
		*x = input_x;
		*y = input_y;
		break;
	}
}

static void add_int_property(struct spa_pod_builder *builder, uint32_t id,
			     int32_t value)
{
	spa_pod_builder_prop(builder, id, 0);
	spa_pod_builder_int(builder, value);
}

static void add_float_property(struct spa_pod_builder *builder, uint32_t id,
			       float value)
{
	spa_pod_builder_prop(builder, id, 0);
	spa_pod_builder_float(builder, value);
}

static int apply_controls(struct app *app)
{
	uint8_t buffer[512];
	struct spa_pod_builder builder = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	struct spa_pod_frame object_frame;

	spa_pod_builder_push_object(&builder, &object_frame,
				    SPA_TYPE_OBJECT_Props, SPA_PARAM_Props);

	if (app->operation == OPERATION_RESET) {
		/* AfModeContinuous and AfMeteringAuto. */
		add_int_property(&builder, app->af_mode_id, 2);
		add_int_property(&builder, app->af_metering_id, 0);
	} else if (app->operation == OPERATION_ADJUST) {
		add_float_property(&builder, app->exposure_value_id,
				   (float)app->exposure_value);
		add_float_property(&builder, app->saturation_id,
				   (float)app->saturation);
		add_float_property(&builder, app->contrast_id,
				   (float)app->contrast);
		add_float_property(&builder, app->sharpness_id,
				   (float)app->sharpness);
	} else if (app->operation == OPERATION_MANUAL_EXPOSURE) {
		/* ExposureTimeModeManual and AnalogueGainModeManual. */
		add_int_property(&builder, app->exposure_time_mode_id, 1);
		add_int_property(&builder, app->exposure_time_id,
				 app->exposure_time_us);
		add_int_property(&builder, app->analogue_gain_mode_id, 1);
		add_float_property(&builder, app->analogue_gain_id,
				   (float)app->analogue_gain);
	} else if (app->operation == OPERATION_AUTO_EXPOSURE) {
		/* ExposureTimeModeAuto and AnalogueGainModeAuto. */
		add_int_property(&builder, app->exposure_time_mode_id, 0);
		add_int_property(&builder, app->analogue_gain_mode_id, 0);
	} else {
		double x = app->focus_x;
		double y = app->focus_y;
		orient_point(app->orientation, &x, &y);

		unsigned int width =
			(unsigned int)llround(app->crop_width * app->focus_size);
		unsigned int height =
			(unsigned int)llround(app->crop_height * app->focus_size);
		width = SPA_CLAMP(width, 32u, app->crop_width);
		height = SPA_CLAMP(height, 32u, app->crop_height);

		int center_x = app->crop_x +
			(int)llround(x * (double)(app->crop_width - 1));
		int center_y = app->crop_y +
			(int)llround(y * (double)(app->crop_height - 1));
		int left = center_x - (int)width / 2;
		int top = center_y - (int)height / 2;
		left = SPA_CLAMP(left, app->crop_x,
				 app->crop_x + (int)app->crop_width - (int)width);
		top = SPA_CLAMP(top, app->crop_y,
				app->crop_y + (int)app->crop_height - (int)height);

		/*
		 * AfModeAuto, AfMeteringWindows, one AfWindows rectangle and
		 * AfTriggerStart. The trigger must be submitted in the same request:
		 * changing to Auto mode alone deliberately leaves autofocus idle.
		 */
		add_int_property(&builder, app->af_mode_id, 1);
		add_int_property(&builder, app->af_metering_id, 1);
		spa_pod_builder_prop(&builder, app->af_windows_id, 0);
		struct spa_pod_frame window_frame;
		spa_pod_builder_push_struct(&builder, &window_frame);
		spa_pod_builder_int(&builder, left);
		spa_pod_builder_int(&builder, top);
		spa_pod_builder_int(&builder, width);
		spa_pod_builder_int(&builder, height);
		spa_pod_builder_pop(&builder, &window_frame);
		add_int_property(&builder, app->af_trigger_id, 0);
	}

	const struct spa_pod *props =
		spa_pod_builder_pop(&builder, &object_frame);
	return pw_node_set_param(app->node, SPA_PARAM_Props, 0, props);
}

static void node_param(void *data, int seq, uint32_t id, uint32_t index,
		       uint32_t next, const struct spa_pod *param)
{
	struct app *app = data;
	uint32_t control_id;
	const char *description = NULL;
	(void)seq;
	(void)index;
	(void)next;

	if (id != SPA_PARAM_PropInfo || !param)
		return;

	if (spa_pod_parse_object(param, SPA_TYPE_OBJECT_PropInfo, NULL,
				 SPA_PROP_INFO_id, SPA_POD_Id(&control_id),
				 SPA_PROP_INFO_description,
				 SPA_POD_OPT_String(&description)) < 0 ||
	    !description)
		return;

	if (spa_streq(description, "AfMode"))
		app->af_mode_id = control_id;
	else if (spa_streq(description, "AfTrigger"))
		app->af_trigger_id = control_id;
	else if (spa_streq(description, "AfMetering"))
		app->af_metering_id = control_id;
	else if (spa_streq(description, "AfWindows"))
		app->af_windows_id = control_id;
	else if (spa_streq(description, "ExposureValue"))
		app->exposure_value_id = control_id;
	else if (spa_streq(description, "Saturation"))
		app->saturation_id = control_id;
	else if (spa_streq(description, "Contrast"))
		app->contrast_id = control_id;
	else if (spa_streq(description, "Sharpness"))
		app->sharpness_id = control_id;
	else if (spa_streq(description, "ExposureTime"))
		app->exposure_time_id = control_id;
	else if (spa_streq(description, "ExposureTimeMode"))
		app->exposure_time_mode_id = control_id;
	else if (spa_streq(description, "AnalogueGain"))
		app->analogue_gain_id = control_id;
	else if (spa_streq(description, "AnalogueGainMode"))
		app->analogue_gain_mode_id = control_id;
}

static void request_controls(struct app *app)
{
	if (app->controls_requested || !app->node)
		return;
	if (app->operation == OPERATION_WAIT_FOCUS) {
		app->stage = STAGE_WAIT_AUTOFOCUS;
		evaluate_wait_result(app);
		return;
	}
	if (app->operation == OPERATION_FOCUS && !app->crop_available)
		return;

	app->controls_requested = true;
	app->stage = STAGE_ENUM_CONTROLS;
	int res = pw_node_enum_params(app->node, 0, SPA_PARAM_PropInfo,
				      0, UINT32_MAX, NULL);
	if (res < 0) {
		finish(app, 4, "failed to enumerate camera controls");
		return;
	}
	app->sync_seq = pw_core_sync(app->core, PW_ID_CORE, app->sync_seq);
}

static void node_info(void *data, const struct pw_node_info *info)
{
	struct app *app = data;
	if (info->props) {
		const char *crop = spa_dict_lookup(info->props, CROP_KEY);
		if (!crop)
			crop = spa_dict_lookup(info->props, CROP_MAXIMUM_KEY);
		if (crop)
			app->crop_available = parse_crop(app, crop);

		const char *orientation = spa_dict_lookup(info->props, ORIENTATION_KEY);
		if (orientation) {
			uint64_t parsed;
			if (parse_uint64(orientation, &parsed) && parsed >= 1 && parsed <= 8)
				app->orientation = (unsigned int)parsed;
		}

		const char *af_state = spa_dict_lookup(info->props, AF_STATE_KEY);
		if (af_state) {
			app->autofocus_state = parse_autofocus_state(af_state);
			app->af_state_available = true;
		}

		const char *af_trigger_generation =
			spa_dict_lookup(info->props, AF_TRIGGER_GENERATION_KEY);
		if (af_trigger_generation) {
			uint64_t parsed;
			if (parse_uint64(af_trigger_generation, &parsed)) {
				app->af_trigger_generation = parsed;
				app->af_trigger_generation_available = true;
			}
		}

		const char *af_state_trigger_generation =
			spa_dict_lookup(info->props,
					AF_STATE_TRIGGER_GENERATION_KEY);
		if (af_state_trigger_generation) {
			uint64_t parsed;
			if (parse_uint64(af_state_trigger_generation, &parsed)) {
				app->af_state_trigger_generation = parsed;
				app->af_state_trigger_generation_available = true;
			}
		}
	}
	if (app->operation == OPERATION_WAIT_FOCUS &&
	    !app->af_state_available) {
		finish(app, 3, "camera stack does not expose autofocus state");
		return;
	}

	request_controls(app);
	if (app->operation == OPERATION_WAIT_FOCUS)
		evaluate_wait_result(app);
	else
		evaluate_autofocus_result(app);
}

static const struct pw_node_events node_events = {
	PW_VERSION_NODE_EVENTS,
	.info = node_info,
	.param = node_param,
};

static void registry_global(void *data, uint32_t id, uint32_t permissions,
			    const char *type, uint32_t version,
			    const struct spa_dict *props)
{
	struct app *app = data;
	(void)permissions;
	if (app->node || !spa_streq(type, PW_TYPE_INTERFACE_Node) || !props)
		return;

	const char *serial = spa_dict_lookup(props, PW_KEY_OBJECT_SERIAL);
	uint64_t parsed_serial;
	if (!serial || !parse_uint64(serial, &parsed_serial) ||
	    parsed_serial != app->target_serial)
		return;

	app->node = pw_registry_bind(app->registry, id, PW_TYPE_INTERFACE_Node,
				     SPA_MIN(version, (uint32_t)PW_VERSION_NODE), 0);
	if (!app->node) {
		finish(app, 4, "failed to bind camera node");
		return;
	}

	pw_node_add_listener(app->node, &app->node_listener, &node_events, app);
}

static const struct pw_registry_events registry_events = {
	PW_VERSION_REGISTRY_EVENTS,
	.global = registry_global,
};

static void core_done(void *data, uint32_t id, int seq)
{
	struct app *app = data;
	if (id != PW_ID_CORE || seq != app->sync_seq)
		return;

	if (app->stage == STAGE_DISCOVER) {
		if (!app->node)
			finish(app, 3, "camera node is not available");
		return;
	}

	if (app->stage == STAGE_ENUM_CONTROLS) {
		if ((app->operation == OPERATION_FOCUS &&
		     (app->af_mode_id == 0 || app->af_metering_id == 0 ||
		      app->af_trigger_id == 0 || app->af_windows_id == 0)) ||
		    (app->operation == OPERATION_RESET &&
		     (app->af_mode_id == 0 || app->af_metering_id == 0))) {
			finish(app, 3, "camera does not support tap-to-focus");
			return;
		}
		if (app->operation == OPERATION_FOCUS &&
		    !app->af_state_available) {
			finish(app, 3,
			       "camera stack does not expose autofocus results");
			return;
		}
		if (app->operation == OPERATION_ADJUST &&
		    (app->exposure_value_id == 0 || app->saturation_id == 0 ||
		     app->contrast_id == 0 || app->sharpness_id == 0)) {
			finish(app, 3, "camera does not support image adjustments");
			return;
		}
		if ((app->operation == OPERATION_MANUAL_EXPOSURE ||
		     app->operation == OPERATION_AUTO_EXPOSURE) &&
		    (app->exposure_time_id == 0 ||
		     app->exposure_time_mode_id == 0 ||
		     app->analogue_gain_id == 0 ||
		     app->analogue_gain_mode_id == 0)) {
			finish(app, 3,
			       "camera does not support manual exposure controls");
			return;
		}

		if (app->operation == OPERATION_FOCUS)
			app->baseline_af_trigger_generation =
				app->af_trigger_generation_available
					? app->af_trigger_generation
					: 0;

		if (apply_controls(app) < 0) {
			finish(app, 4, "camera rejected controls");
			return;
		}
		app->stage = STAGE_APPLY_CONTROLS;
		app->sync_seq = pw_core_sync(app->core, PW_ID_CORE, app->sync_seq);
		return;
	}

	if (app->stage == STAGE_APPLY_CONTROLS &&
	    app->operation == OPERATION_FOCUS) {
		app->stage = STAGE_WAIT_AUTOFOCUS;
		evaluate_autofocus_result(app);
		return;
	}

	if (app->stage == STAGE_WAIT_AUTOFOCUS) {
		evaluate_wait_result(app);
		return;
	}

	finish(app, 0, NULL);
}

static void core_error(void *data, uint32_t id, int seq, int res,
		       const char *message)
{
	struct app *app = data;
	(void)id;
	(void)seq;
	if (res < 0)
		finish(app, 4, message);
}

static const struct pw_core_events core_events = {
	PW_VERSION_CORE_EVENTS,
	.done = core_done,
	.error = core_error,
};

static void timeout(void *data, uint64_t expirations)
{
	struct app *app = data;
	(void)expirations;
	if (app->operation == OPERATION_FOCUS &&
	    app->stage == STAGE_WAIT_AUTOFOCUS) {
		finish(app, 4,
		       app->expected_af_trigger_generation == 0
			       ? "timed out waiting for autofocus trigger acknowledgement"
			       : "timed out waiting for autofocus result");
		return;
	}
	if (app->operation == OPERATION_WAIT_FOCUS &&
	    app->stage == STAGE_WAIT_AUTOFOCUS) {
		finish(app, 4, "timed out waiting for autofocus to settle");
		return;
	}
	finish(app, 4, "timed out waiting for PipeWire");
}

static void usage(const char *program)
{
	fprintf(stderr,
		"usage: %s focus SERIAL X Y SIZE\n"
		"       %s wait SERIAL\n"
		"       %s reset SERIAL\n"
		"       %s adjust SERIAL EXPOSURE SATURATION CONTRAST SHARPNESS\n"
		"       %s manual SERIAL EXPOSURE_US ANALOGUE_GAIN\n"
		"       %s auto SERIAL\n",
		program, program, program, program, program, program);
}

int main(int argc, char **argv)
{
	struct app app = {
		.orientation = 1,
		.result = 4,
	};

	if (argc >= 2 && spa_streq(argv[1], "focus")) {
		if (argc != 6 || !parse_uint64(argv[2], &app.target_serial) ||
		    !parse_double(argv[3], &app.focus_x) ||
		    !parse_double(argv[4], &app.focus_y) ||
		    !parse_double(argv[5], &app.focus_size) ||
		    app.focus_x < 0.0 || app.focus_x > 1.0 ||
		    app.focus_y < 0.0 || app.focus_y > 1.0 ||
		    app.focus_size < 0.05 || app.focus_size > 0.5) {
			usage(argv[0]);
			return 2;
		}
		app.operation = OPERATION_FOCUS;
	} else if (argc == 3 && spa_streq(argv[1], "wait") &&
		   parse_uint64(argv[2], &app.target_serial)) {
		app.operation = OPERATION_WAIT_FOCUS;
	} else if (argc == 3 && spa_streq(argv[1], "reset") &&
		   parse_uint64(argv[2], &app.target_serial)) {
		app.operation = OPERATION_RESET;
	} else if (argc == 7 && spa_streq(argv[1], "adjust") &&
		   parse_uint64(argv[2], &app.target_serial) &&
		   parse_double(argv[3], &app.exposure_value) &&
		   parse_double(argv[4], &app.saturation) &&
		   parse_double(argv[5], &app.contrast) &&
		   parse_double(argv[6], &app.sharpness) &&
		   app.exposure_value >= -1.0 && app.exposure_value <= 1.0 &&
		   app.saturation >= 0.0 && app.saturation <= 2.0 &&
		   app.contrast >= 0.0 && app.contrast <= 2.0 &&
		   app.sharpness >= 0.0 && app.sharpness <= 2.0) {
		app.operation = OPERATION_ADJUST;
	} else if (argc == 5 && spa_streq(argv[1], "manual") &&
		   parse_uint64(argv[2], &app.target_serial) &&
		   parse_int32(argv[3], &app.exposure_time_us) &&
		   parse_double(argv[4], &app.analogue_gain) &&
		   app.exposure_time_us > 0 &&
		   app.analogue_gain >= 0.1 && app.analogue_gain <= 256.0) {
		app.operation = OPERATION_MANUAL_EXPOSURE;
	} else if (argc == 3 && spa_streq(argv[1], "auto") &&
		   parse_uint64(argv[2], &app.target_serial)) {
		app.operation = OPERATION_AUTO_EXPOSURE;
	} else {
		usage(argv[0]);
		return 2;
	}

	pw_init(&argc, &argv);
	app.loop = pw_main_loop_new(NULL);
	if (!app.loop)
		goto cleanup;

	app.context = pw_context_new(pw_main_loop_get_loop(app.loop), NULL, 0);
	if (!app.context)
		goto cleanup;

	app.core = pw_context_connect(app.context, NULL, 0);
	if (!app.core) {
		fprintf(stderr, "advanced-snapshot-focus-control: cannot connect to PipeWire: %s\n",
			strerror(errno));
		goto cleanup;
	}

	pw_core_add_listener(app.core, &app.core_listener, &core_events, &app);
	app.registry = pw_core_get_registry(app.core, PW_VERSION_REGISTRY, 0);
	if (!app.registry)
		goto cleanup;
	pw_registry_add_listener(app.registry, &app.registry_listener,
				 &registry_events, &app);

	app.timer = pw_loop_add_timer(pw_main_loop_get_loop(app.loop), timeout, &app);
	if (!app.timer)
		goto cleanup;
	/* A cold simple-IPA autofocus scan deliberately waits for sensor
	 * statistics to become valid before traversing the actuator.  Give both
	 * the explicit tap operation and the still-capture settle barrier enough
	 * time to finish that bounded scan; the registry/control operations remain
	 * short. */
	struct timespec timeout_value = {
		.tv_sec = (app.operation == OPERATION_FOCUS ||
			   app.operation == OPERATION_WAIT_FOCUS) ? 15 : 3,
	};
	struct timespec interval = { 0 };
	pw_loop_update_timer(pw_main_loop_get_loop(app.loop), app.timer,
			     &timeout_value, &interval, false);

	app.stage = STAGE_DISCOVER;
	app.sync_seq = pw_core_sync(app.core, PW_ID_CORE, 0);
	pw_main_loop_run(app.loop);

cleanup:
	if (app.timer)
		pw_loop_destroy_source(pw_main_loop_get_loop(app.loop), app.timer);
	if (app.node)
		pw_proxy_destroy((struct pw_proxy *)app.node);
	if (app.registry)
		pw_proxy_destroy((struct pw_proxy *)app.registry);
	if (app.core)
		pw_core_disconnect(app.core);
	if (app.context)
		pw_context_destroy(app.context);
	if (app.loop)
		pw_main_loop_destroy(app.loop);
	pw_deinit();

	return app.result;
}
