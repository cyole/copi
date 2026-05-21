using System.Security.Cryptography;

namespace Copi.Windows.Services;

public static class SecretGenerator
{
    public static string Generate()
    {
        Span<byte> bytes = stackalloc byte[24];
        RandomNumberGenerator.Fill(bytes);
        return Convert.ToBase64String(bytes);
    }
}
