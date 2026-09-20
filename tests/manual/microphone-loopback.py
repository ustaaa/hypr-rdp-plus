"""Local end-to-end synthetic mic test. Never captures a physical microphone.
Run inside the Omarchy session. Server must listen on127.0.0.1:3391 with /tmp/hypr-mic-test/config.toml.
"""
import os, subprocess as sp, time, pathlib, tomllib, json, signal, wave, math, array
root=pathlib.Path('/tmp/hypr-mic-test')
c=tomllib.loads(pathlib.Path(os.environ.get('MIC_TEST_CONFIG',str(root/'config.toml'))).read_text())
address=os.environ.get('MIC_TEST_ADDRESS','127.0.0.1:3391')
mic_rate=os.environ.get('MIC_TEST_RATE','48000')
mic_channels=os.environ.get('MIC_TEST_CHANNELS','1')
procs=[]; module=None
try:
 module=sp.check_output(['pactl','load-module','module-null-sink','sink_name=hypr_mic_test','rate=48000','channels=1'],text=True).strip()
 tone=sp.Popen(['ffmpeg','-nostdin','-loglevel','error','-re','-f','lavfi','-i','sine=frequency=440:sample_rate=48000','-ac','1','-f','pulse','-device','hypr_mic_test','RDP synthetic microphone test'],stdout=sp.DEVNULL,stderr=(root/'tone.log').open('w'));procs.append(tone)
 client=sp.Popen(['wlfreerdp3','/v:'+address,'/u:'+c['username'],'/p:'+c['password'],'/cert:ignore','/size:800x600','/gfx:AVC420',f'/microphone:sys:pulse,dev:hypr_mic_test.monitor,format:1,rate:{mic_rate},channel:{mic_channels}','/log-level:INFO'],env=dict(os.environ,LIBVA_DRIVER_NAME='none'),stdout=(root/'client.log').open('w'),stderr=sp.STDOUT);procs.append(client)
 for _ in range(100):
  sources=json.loads(sp.check_output(['pactl','-f','json','list','sources']))
  if any(s['name']=='hypr-rdp-microphone' for s in sources):break
  if client.poll() is not None:raise RuntimeError('client exited; see client.log')
  time.sleep(.1)
 else:raise RuntimeError('RDP microphone source did not appear')
 # PipeWire raw signed PCM output; no actual voice is recorded.
 out=(root/'received.pcm').open('wb')
 recorder=sp.Popen(['pw-record','--raw','--target','hypr-rdp-microphone','--rate','48000','--channels','1','--format','s16','-'],stdout=out,stderr=(root/'record.log').open('w'));procs.append(recorder)
 time.sleep(3);recorder.send_signal(signal.SIGINT);recorder.wait(timeout=5);out.close()
 samples=array.array('h');samples.frombytes((root/'received.pcm').read_bytes())
 rms=(sum(x*x for x in samples)/max(1,len(samples)))**.5
 # Estimate dominant tone by correlation; reject mere random noise.
 power=sum(x*x for x in samples)
 real=sum(x*math.cos(2*math.pi*440*i/48000) for i,x in enumerate(samples))
 imag=sum(x*math.sin(2*math.pi*440*i/48000) for i,x in enumerate(samples))
 ratio=2*(real*real+imag*imag)/max(1,len(samples)*power)
 result={'sample_count':len(samples),'rms':rms,'tone_440hz_fraction':ratio}
 print(json.dumps(result));(root/'result.json').write_text(json.dumps(result,indent=2))
 assert len(samples)>48000 and rms>100 and ratio>.5, 'No valid transferred test tone'
finally:
 for p in reversed(procs):
  if p.poll() is None:
   p.terminate()
   try:p.wait(timeout=5)
   except sp.TimeoutExpired:p.kill();p.wait()
 if module:sp.run(['pactl','unload-module',module],check=False)
