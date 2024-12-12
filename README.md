# Ta .3d0 to .obj converter

To use this tool you do: 

```
./ta-3do-to-obj -i ./input.3do -g ./path/to/texture_gafs/ -o ./output.obj
```

This will convert the `input.3do` into `output.obj`. It will also find textures used in the 3do file and extract them as .bmp files and put them in ./textures/