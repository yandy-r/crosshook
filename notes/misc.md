## Ideas

### UI Enhancements (Install Page)

I recently did a pretty large overhaul of the Profile and Launch pages in PR 134 and 136.

- tabbed interface
- cover art
- other ui enhancements

I want to do the same to the Install page.

- Tabbed interface
- All sections should be a tab in a single interface
- Maintain styling consistency so make sure all containers/tabs across pages have consistent styling

### GPU sections

```text
 AMD (RX 9070 XT):
  PROTON_NO_STEAMINPUT=1 PROTON_PREFER_SDL=1 PROTON_NO_WM_DECORATION=1 PROTON_ENABLE_HDR=1 PROTON_ENABLE_WAYLAND=1 PROTON_LOCAL_SHADER_CACHE=1 DXVK_ASYNC=1
  VKD3D_CONFIG=dxr PROTON_FSR4_RDNA3_UPGRADE=1 SteamDeck=1 DXVK_FILTER_DEVICE_NAME="AMD" VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json mangohud
  game-performance %command%

  NVIDIA (RTX 5070):
  PROTON_NO_STEAMINPUT=1 PROTON_PREFER_SDL=1 PROTON_NO_WM_DECORATION=1 PROTON_ENABLE_HDR=1 PROTON_ENABLE_WAYLAND=1 PROTON_LOCAL_SHADER_CACHE=1 DXVK_ASYNC=1
  VKD3D_CONFIG=dxr PROTON_XESS_UPGRADE=1 PROTON_DLSS_INDICATOR=1 SteamDeck=1 DXVK_FILTER_DEVICE_NAME="NVIDIA" VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nvidia_icd.json
  mangohud game-performance %command%
```
