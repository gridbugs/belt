#version 150 core

in vec2 a_CornerZeroToOne;

uniform Properties {
    vec2 u_PlayerPositionInPixels;
    float u_Zoom;
};
uniform PropertiesStatic {
    vec2 u_WindowSizeInPixels;
    vec2 u_InputSizeInPixels;
};

out vec2 v_TexCoord;

void main() {

    vec2 player_position = vec2(u_PlayerPositionInPixels.x, u_WindowSizeInPixels.y - u_PlayerPositionInPixels.y);
    vec2 top_left_corner = player_position - u_WindowSizeInPixels / (u_Zoom * 2);
    vec2 corner = top_left_corner + (u_WindowSizeInPixels / u_Zoom) * a_CornerZeroToOne;
    v_TexCoord = corner / u_InputSizeInPixels;

    vec2 screen_coord = vec2(
        a_CornerZeroToOne.x * 2 - 1,
        a_CornerZeroToOne.y * 2 - 1);

    gl_Position = vec4(screen_coord, 0, 1);
}
