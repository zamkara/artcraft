> **Unofficial Arch Linux builds of [ArtCraft](https://github.com/storytold/artcraft).**
> This repository only builds and publishes Arch Linux releases of the upstream project, which does not currently provide them.
> It is not affiliated with or endorsed by the upstream authors. For the source code, issues, and development, go to [storytold/artcraft](https://github.com/storytold/artcraft).

**artcraft_is_easy_fun_and_yours.mp4**

[https://github.com/user-attachments/assets/b4e24c27-d87d-4fd1-8599-dc0d0b8af48d](https://github.com/user-attachments/assets/b4e24c27-d87d-4fd1-8599-dc0d0b8af48d)

The IDE for artists.

[![Discord](https://img.shields.io/discord/1359579021108842617?style=for-the-badge&label=discord&color=ffffff&logo=discord&logoColor=ffffff)](https://discord.gg/artcraft) [![YouTube](https://img.shields.io/youtube/channel/subscribers/UCdjY4VG0ntoGwFsKZO4sVWA?style=for-the-badge&logo=YouTube)](https://www.youtube.com/@OfficialArtCraftStudios) [![X](https://img.shields.io/twitter/follow/get_artcraft?style=for-the-badge&label=follow&logo=x&logoColor=ffffff&color=ffffff)](https://x.com/intent/follow?screen_name=get_artcraft) [![LinkedIn](https://img.shields.io/badge/linkedin--0A66C2?style=for-the-badge)](https://www.linkedin.com/company/artcraft-ai)

---

# ArtCraft

ArtCraft is the IDE for interactive AI image and video creation.
We turn prompting into *crafting*, so your ideas become a form of tangible expression and computing.
This is Adobe Photoshop for everyone, and we're giving away the source code!

## Show, Don't Tell: Advanced Crafting Features

Text-to-image is great, but artists *need control*. It's important to know what your image will look like before you generate it, and it's vitally important to achieve consistency and repeatability.

| Feature                           | Demo + Explanation |
| --------------------------------- | ------------------ |
| **Image to Location**             | [![Video](https://github.com/user-attachments/assets/21f103e3-cc19-4882-a630-9caa1b76ae31)](https://github.com/user-attachments/assets/21f103e3-cc19-4882-a630-9caa1b76ae31) Placing virtual actors into physical environments establishes single-location consistency. You can film multiple shots within a room without having things disappear. |
| **3D Image Compositing**          | [![Video](https://github.com/user-attachments/assets/f93a616f-571d-474e-bcc0-53736de7303d)](https://github.com/user-attachments/assets/f93a616f-571d-474e-bcc0-53736de7303d) Use images (backdrops, foreground elements, props, etc.) in scenes with depth and blend them naturally together. Just a couple of images usually leads to great compositions. |
| **2D Image Compositing**          | [![Video](https://github.com/user-attachments/assets/d6f99391-e496-4c62-9e37-29734ba5f899)](https://github.com/user-attachments/assets/d6f99391-e496-4c62-9e37-29734ba5f899) Use images, background removal, layers, and simple drawing tools to precisely compose a scene. |
| **Image to 3D Mesh**              | [![Video](https://github.com/user-attachments/assets/600a405c-e360-48c1-9b42-6e657ae6243b)](https://github.com/user-attachments/assets/600a405c-e360-48c1-9b42-6e657ae6243b) It's almost impossible to lay out complicated objects or block complicated scenes; turning images into 3D helps position elements exactingly and intentionally. |
| **Character Posing**              | [![Video](https://raw.githubusercontent.com/storytold/github-media/22a27373707b13f48fc56405f1f89ca143c43d5d/character-posing.webp)](https://raw.githubusercontent.com/storytold/github-media/22a27373707b13f48fc56405f1f89ca143c43d5d/character-posing.webp) You can dynamically pose your characters to achieve the precise character, scene, and camera blocking before calling "action". |
| **Scene Blocking w/ Kit Bashing** | [![Video](https://github.com/user-attachments/assets/eef025ac-0346-4a46-a023-d48e23629eb5)](https://github.com/user-attachments/assets/eef025ac-0346-4a46-a023-d48e23629eb5) Use 3D asset kits to precisely block out your scene: get the correct angles, object positions, and rich depth layering you can't with text prompting. |
| **Character Identity Transfer**   | [![Video](https://github.com/user-attachments/assets/629119ee-8c76-4a83-9827-8c6c995a3ec1)](https://github.com/user-attachments/assets/629119ee-8c76-4a83-9827-8c6c995a3ec1) Use mannequins as simple 3D ControlNets for posing any character. |
| **Background Removal**            | [![Video](https://github.com/user-attachments/assets/90c65057-5531-404f-83af-b34e66e24ec1)](https://github.com/user-attachments/assets/90c65057-5531-404f-83af-b34e66e24ec1) Remove backgrounds from images to make them useful in 2D or 3D compositing. They can be props, layers, or backdrops. |
| **Mixed Asset Crafting**          | [![Video](https://raw.githubusercontent.com/storytold/github-media/main/ship-editing.gif)](https://raw.githubusercontent.com/storytold/github-media/main/ship-editing.gif) You can use image cutouts, worlds, and simple 3D meshes all together to precisely and intentionally lay out your scenes. |
| **Scene Blocking**                | (preview coming soon) |
| **Canvas Editing**                | (preview coming soon) |
| **Scene Relighting**              | (preview coming soon) |

Note: all of the above videos were generated for free with Grok Video; the cost to build this README was negligible.

## Quick and Easy Prompting

We haven't abandoned text-to-asset generation for quick prototyping and ideation. We support every popular workflow in a first class fashion.

| Feature               | Demo + Explanation |
| --------------------- | ------------------ |
| **Text to Image**     | [![Video](https://github.com/user-attachments/assets/9cc289cd-faf4-4eaf-aed2-21134cce127c)](https://github.com/user-attachments/assets/9cc289cd-faf4-4eaf-aed2-21134cce127c) Text prompt over a dozen different image models. |
| **Image Editing**     | [![Video](https://github.com/user-attachments/assets/a06fa6ad-936c-42d0-8767-48fdbb8ff141)](https://github.com/user-attachments/assets/a06fa6ad-936c-42d0-8767-48fdbb8ff141) Edit with Nano Banana Pro and GPT Image 1.5. |
| **Image Editing**     | [![Video](https://github.com/user-attachments/assets/f036e08a-f3a6-417a-98ee-ec7f04b2b5ff)](https://github.com/user-attachments/assets/f036e08a-f3a6-417a-98ee-ec7f04b2b5ff) Use inpainting, drawing, masking, etc. to edit images. |
| **Image to Video**    | [![Video](https://github.com/user-attachments/assets/2bc6c592-511e-4fba-b40f-03c96699b7f7)](https://github.com/user-attachments/assets/2bc6c592-511e-4fba-b40f-03c96699b7f7) Image to video with lots of different options and controls. |
| **Image Inpainting**  | (preview coming soon) |
| **Image Ingredients** | (preview coming soon) |

## Models and Providers Supported within Artcraft

| Provider   | Features |
| ---------- | -------- |
| Artcraft   | Nano Banana, Nano Banana Pro, GPT-Image-1 / 1.5, Seedance 1.0 Lite / 1.5 Pro / 2.0, Seedream 4 / 4.5, Flux 1.1 / Kontext, Veo 2 / 3 / 3.1, Kling 1.6 / 2.1 / 2.5 / 2.6, Sora 2 / Pro, Hunyuan 3d 2 / 3 |
| Grok       | Grok Imagine, Grok Video |
| Midjourney | Image Gen (all versions) |
| Sora       | Sora 1, Sora 2, GPT-Image-1 |
| WorldLabs  | Marble (Gaussian Splat World Generation) |

We're going to be adding the following providers soon: Kling (via Kling website accounts), Google (via API keys),
Runway (via website account), Luma (via website account).

We're potentially interested in adding other aggregators for those who already have subscriptions and credits at
those providers, for example: OpenArt, FreePik, etc.

## Downloads

- **Arch Linux:** grab the latest build from the [Releases page of this repository](https://github.com/zamkara/artcraft/releases)
- [Visit the upstream website for the stable Windows and MacOS releases](https://getartcraft.com/)
- Or you can grab a [more recent Windows and MacOS build directly from upstream](https://github.com/storytold/artcraft/releases)

## Documentation

- [developer documentation](https://github.com/storytold/artcraft/blob/main/_docs)
- [tools, scripts, misc](https://github.com/storytold/artcraft/blob/main/script)
- [upstream license](https://github.com/storytold/artcraft/blob/main/LICENSE.md)
- [roadmap](https://github.com/storytold/artcraft/blob/main/ROADMAP.md)

## License

ArtCraft itself remains under its [upstream license](https://github.com/storytold/artcraft/blob/main/LICENSE.md).
The build setup in this repository is released under the [Almatera Incubator License](LICENSE).
