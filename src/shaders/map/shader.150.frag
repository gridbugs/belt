#version 150 core

in vec2 v_TexCoord;
out vec4 TargetColour;
out vec4 TargetVisibility;
uniform sampler2D t_Image;

void main() {
    vec4 colour = texture(t_Image, v_TexCoord);
    TargetColour = colour;
    if (colour.r > 0.99 && colour.g > 0.99 && colour.b > 0.99) {
        TargetVisibility = vec4(1,1,1,1);
    } else {
        TargetVisibility = vec4(0,0,0,1);
    }
}
