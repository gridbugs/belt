#version 150 core

in vec2 v_TexCoord;
in vec2 v_PixelCoord;

out vec4 Target0;
uniform sampler2D t_Colour;
uniform sampler2D t_Visibility;

uniform Properties {
    vec2 u_EyePositionInPixels;
};

uniform PropertiesStatic {
    vec2 u_WindowSizeInPixels;
};

const float JUST_ABOVE_ZERO = 0.01;
const float JUST_UNDER_ONE  = 0.99;

bool is_roughly_integer(float f) {
    f = fract(f);
    return f < JUST_ABOVE_ZERO || f > JUST_UNDER_ONE;
}

bool is_transparent(vec4 colour) {
    return colour.r > JUST_UNDER_ONE;
}

bool is_opaque(vec4 colour) {
    return colour.r < JUST_ABOVE_ZERO;
}

struct Lod {
    float exponent;
    float pixel_size;
};

const uint EDGE_AXIS_X = 1u;
const uint EDGE_AXIS_Y = 2u;
const uint EDGE_AXIS_BOTH = 3u;

const Lod INITIAL_LOD = Lod(4, 16);
const uint MAX_RAY_TRACE_DEPTH = 1000u;

bool is_pixel_visible(
        vec2 px_start_coord,
        vec2 px_light_coord,
        sampler2D world,
        vec2 world_size,
        const Lod max_lod) {

    float px_start_to_light_distance = distance(px_start_coord, px_light_coord);
    vec2 px_coord = px_start_coord;
    vec2 px_to_light = px_light_coord - px_coord;
    Lod lod = max_lod;

    for (uint i = 0u; i < MAX_RAY_TRACE_DEPTH; i++) {
        vec2 scaled_coord = px_coord / lod.pixel_size;
        vec2 scaled_far_corner = vec2(0, 0);
        if (px_light_coord.x > px_start_coord.x) {
            scaled_far_corner.x = floor(scaled_coord.x + 1);
        } else {
            scaled_far_corner.x = ceil(scaled_coord.x - 1);
        }
        if (px_light_coord.y > px_start_coord.y) {
            scaled_far_corner.y = floor(scaled_coord.y + 1);
        } else {
            scaled_far_corner.y = ceil(scaled_coord.y - 1);
        }
        vec2 px_far_corner = scaled_far_corner * lod.pixel_size;


        uint edge_axis = 0u;
        vec2 px_next_coord = px_coord;
        if (px_to_light.x == 0) {
            px_next_coord.y = px_far_corner.y;
            edge_axis = EDGE_AXIS_Y;
        } else if (px_to_light.y == 0) {
            px_next_coord.x = px_far_corner.x;
            edge_axis = EDGE_AXIS_X;
        } else {
            vec2 edge_mult = (px_far_corner - px_coord) / px_to_light;
            if (edge_mult.x < edge_mult.y) {
                px_next_coord = vec2(px_far_corner.x, px_coord.y + edge_mult.x * px_to_light.y);
                edge_axis = EDGE_AXIS_X;
            } else if (edge_mult.y < edge_mult.x) {
                px_next_coord = vec2(px_coord.x + edge_mult.y * px_to_light.x, px_far_corner.y);
                edge_axis = EDGE_AXIS_Y;
            } else {
                px_next_coord = px_far_corner;
                edge_axis = EDGE_AXIS_BOTH;
            }
        }

        vec2 px_sample_coord = (px_coord + px_next_coord) / 2;
        vec4 sample_colour = textureLod(world, px_sample_coord / world_size, lod.exponent);

        if (is_opaque(sample_colour)) {
            return false;
        } else if (is_transparent(sample_colour)) {
            if (distance(px_start_coord, px_next_coord) > px_start_to_light_distance) {
                return true;
            } else {
                px_coord = px_next_coord;
                if (edge_axis == EDGE_AXIS_BOTH) {
                    while (lod.exponent < max_lod.exponent) {
                        float next_pixel_size = lod.pixel_size * 2;
                        if (is_roughly_integer(px_coord.x / next_pixel_size) &&
                                is_roughly_integer(px_coord.y / next_pixel_size)) {
                            lod.exponent += 1;
                            lod.pixel_size = next_pixel_size;
                        } else {
                            break;
                        }
                    }
                } else {
                    float axis = 0;
                    if (edge_axis == EDGE_AXIS_X) {
                        axis = px_coord.x;
                    } else {
                        axis = px_coord.y;
                    }
                    while (lod.exponent < max_lod.exponent) {
                        float next_pixel_size = lod.pixel_size * 2;
                        if (is_roughly_integer(axis / next_pixel_size)) {
                            lod.exponent += 1;
                            lod.pixel_size = next_pixel_size;
                        } else {
                            break;
                        }
                    }
                }
            }
        } else {
            lod.exponent -= 1;
            lod.pixel_size /= 2;
        }
    }
    return false;
}

void main() {
    if (!is_pixel_visible(
                v_PixelCoord,
                u_EyePositionInPixels,
                t_Visibility,
                u_WindowSizeInPixels,
                INITIAL_LOD)) {
        Target0 = vec4(0,0,0,1);
    } else {
        vec4 colour = texture(t_Colour, v_TexCoord);
        Target0 = colour;
    }
}
