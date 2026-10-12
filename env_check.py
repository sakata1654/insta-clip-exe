import importlib.util as u, json
mods = ['faster_whisper', 'whisper', 'demucs', 'torch', 'mediapipe', 'cv2', 'moviepy', 'imageio_ffmpeg', 'yt_dlp', 'arabic_reshaper', 'bidi', 'ctranslate2']
r = {m: bool(u.find_spec(m)) for m in mods}
cuda = False
try:
    import ctranslate2
    cuda = ctranslate2.get_cuda_device_count() > 0
except Exception:
    try:
        import torch
        cuda = bool(torch.cuda.is_available())
    except Exception:
        cuda = False
r['cuda'] = cuda
print(json.dumps(r))
