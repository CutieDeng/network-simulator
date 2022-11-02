import mysocket 

me = mysocket.MySocket() 
me.bind(("127.6.6.6", 6665)) 

while True: 
    (d, addr) = me.recvfrom(16) 
    print (f"recv [{d}] from {addr}")