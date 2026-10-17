# Cohere Transcribe (English / Arabic) with VAD-based timing.
# Cohere's models give no timestamps, so speech is first cut with a voice-activity detector
# (this also stops the "eager to transcribe silence" hallucinations the model card warns about).
import sys, json, os
import numpy as np

def vad_segments(audio, sr):
    try:
        from faster_whisper.vad import get_speech_timestamps, VadOptions
        ts = get_speech_timestamps(audio, VadOptions(min_silence_duration_ms=400, speech_pad_ms=200, max_speech_duration_s=20))
        segs = [(t['start'] / sr, t['end'] / sr) for t in ts]
    except Exception as e:
        print(f"VAD unavailable ({e}); using fixed windows", flush=True)
        n = len(audio) / sr
        segs = [(float(i), float(min(n, i + 15.0))) for i in np.arange(0, n, 15.0)]
    # merge neighbours that are very close, and glue tiny pieces to a neighbour (max ~12 s)
    out = []
    for s, e in segs:
        if out and (s - out[-1][1] < 0.35 and e - out[-1][0] < 12.0):
            out[-1] = (out[-1][0], e)
        elif out and (e - s < 0.8 and e - out[-1][0] < 12.0):
            out[-1] = (out[-1][0], e)
        else:
            out.append((s, e))
    return out

def main():
    a = json.load(open(sys.argv[1], encoding='utf-8'))
    import soundfile as sf
    audio, sr = sf.read(a['audio'], dtype='float32')
    if audio.ndim > 1:
        audio = audio.mean(axis=1)
    segs = vad_segments(audio, sr)
    print(f"{len(segs)} speech segments", flush=True)
    import torch
    from transformers import AutoProcessor, CohereAsrForConditionalGeneration
    mid = a['model_id']
    tok = os.environ.get('HF_TOKEN') or None
    processor = AutoProcessor.from_pretrained(mid, token=tok)
    try:
        model = CohereAsrForConditionalGeneration.from_pretrained(mid, device_map="auto", token=tok, dtype=torch.bfloat16 if torch.cuda.is_available() else torch.float32)
    except TypeError:
        model = CohereAsrForConditionalGeneration.from_pretrained(mid, device_map="auto", token=tok)
    print(f"MODEL {mid.split('/')[-1]} on {model.device}", flush=True)
    lang = a['lang']
    out, B = [], 8
    for i in range(0, len(segs), B):
        part = segs[i:i + B]
        chunk = [audio[int(s * sr):int(e * sr)] for s, e in part]
        texts = []
        try:
            inputs = processor(chunk, sampling_rate=16000, return_tensors="pt", language=lang)
            aci = inputs.get("audio_chunk_index")
            inputs.to(model.device, dtype=model.dtype)
            res = model.generate(**inputs, max_new_tokens=400)
            dec = processor.decode(res, skip_special_tokens=True, audio_chunk_index=aci, language=lang)
            texts = [dec] if isinstance(dec, str) else list(dec)
        except Exception as e:
            print(f"batch failed ({e}); retrying one by one", flush=True)
        if len(texts) != len(chunk):
            texts = []
            for c in chunk:
                inputs = processor(c, sampling_rate=16000, return_tensors="pt", language=lang)
                inputs.to(model.device, dtype=model.dtype)
                res = model.generate(**inputs, max_new_tokens=400)
                texts.append(processor.decode(res, skip_special_tokens=True))
        for (s, e), t in zip(part, texts):
            t = (t if isinstance(t, str) else str(t)).strip()
            if t:
                out.append({"s": s, "e": e, "t": t})
        print(f"progress = {min(99, int((i + len(part)) / len(segs) * 100))}%", flush=True)
    json.dump(out, open(a['out'], 'w', encoding='utf-8'), ensure_ascii=False)
    print("DONE", flush=True)

main()
