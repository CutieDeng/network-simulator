import socket

class MySocket: 
    def __init__ (self): 
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM) 
    def bind(self, p): 
        return self.socket.bind(p) 
    def sendto(self, data, addr): 
        h1 = socket.inet_aton(addr[0])
        h2 = addr[1].to_bytes(2, 'little')
        sdata = h1 + h2 + data 
        return self.socket.sendto(sdata, ("127.67.117.116", 52736))
    def recvfrom(self, buffer_size): 
        (d, a) = self.socket.recvfrom(buffer_size + 6) 
        if a != ("127.67.117.116", 52736): 
            print ("unexpected message receive ")
            return 
        actuala = socket.inet_ntoa(d[:4])
        actualp = int.from_bytes(d[4:6], "little")
        dd = d[6:]
        return (dd, (actuala, actualp))