#version 150 core

in vec2 a_CornerZeroToOne;
in vec2 i_PositionOfCentreInPixels;
in vec2 i_DimensionsInPixels;
in vec2 i_FacingVector;
in vec2 i_SpritePositionOfTopLeftInPixels;
in vec2 i_SpriteDimensionsInPixels;

uniform Properties {
    vec2 u_WindowSizeInPixels;
    vec2 u_SpriteSheetSizeInPixels;
};

out vec2 v_SpriteSheetSampleCoord;

void main() {

    vec2 right_facing_vector = vec2(-i_FacingVector.y, i_FacingVector.x);

    vec2 pixel_offset_from_centre = i_DimensionsInPixels / 2 - a_CornerZeroToOne * i_DimensionsInPixels;
    vec2 rotated_pixel_offset_from_centre =
        pixel_offset_from_centre.y * i_FacingVector -
        pixel_offset_from_centre.x * right_facing_vector;
    vec2 pixel_coord = i_PositionOfCentreInPixels + rotated_pixel_offset_from_centre;

    vec2 screen_coord = vec2(
        pixel_coord.x / u_WindowSizeInPixels.x * 2 - 1,
        1 - pixel_coord.y / u_WindowSizeInPixels.y * 2);

    v_SpriteSheetSampleCoord =
        (i_SpritePositionOfTopLeftInPixels +
        i_SpriteDimensionsInPixels * a_CornerZeroToOne) / u_SpriteSheetSizeInPixels;

    gl_Position = vec4(screen_coord, 0, 1);
}
