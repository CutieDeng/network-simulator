import mysocket 

me = mysocket.MySocket() 
me.bind(("127.4.5.6", 4444)) 

while True: 
    (d, addr) = me.recvfrom(16) 
    print (f"recv [{d}] from {addr}")