import pathlib,subprocess,sys,tempfile,socket,select,os
path=pathlib.Path('/state/runtime/peercred-namespace-review')
path.mkdir(mode=0o700)
server='''import socket,struct,sys
s=socket.socket(socket.AF_UNIX);s.bind(sys.argv[1]);s.listen(1);print("ready",flush=True)
c,_=s.accept();print(struct.unpack("3i",c.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12)),flush=True)
'''
cmd=['/usr/bin/bwrap','--unshare-all','--die-with-parent','--new-session','--cap-drop','ALL','--ro-bind','/usr','/usr','--symlink','usr/lib','/lib','--symlink','usr/lib','/lib64','--symlink','usr/bin','/bin','--bind','/state','/state','--proc','/proc','--dev','/dev','--tmpfs','/tmp','--clearenv','--setenv','PATH','/usr/bin','--','/usr/bin/python3','-B','-c',server,str(path/'socket')]
p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
try:
 if not select.select([p.stdout],[],[],5)[0]:raise RuntimeError('nested readiness timeout')
 print('inner:',p.stdout.readline().strip())
 c=socket.socket(socket.AF_UNIX);c.settimeout(2);c.connect(str(path/'socket'))
 out,err=p.communicate(timeout=5);print('outer pid/uid:',os.getpid(),os.getuid());print('peercred observed by child namespace:',out.strip());print('exit',p.returncode,'error',err)
finally:
 if p.poll() is None:p.kill();p.wait()
