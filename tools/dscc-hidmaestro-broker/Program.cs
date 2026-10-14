using System.Collections.Concurrent;
using System.Reflection;
using System.Security.Principal;
using System.Text.Json;
using System.Text.Json.Serialization;

const string Protocol = "dev.dscc.hidmaestro-broker.v1";

var broker = new HidMaestroBroker();
while (Console.ReadLine() is { } line)
{
    if (string.IsNullOrWhiteSpace(line)) continue;
    BrokerRequest? request;
    try
    {
        request = JsonSerializer.Deserialize<BrokerRequest>(line, JsonOptions.Default);
    }
    catch
    {
        continue;
    }

    if (request is null || request.Protocol != Protocol) continue;
    if (request.Command == "update")
    {
        broker.Update(request);
        continue;
    }

    var response = broker.Handle(request);
    Console.WriteLine(JsonSerializer.Serialize(response, JsonOptions.Default));
    Console.Out.Flush();
    if (request.Command == "shutdown") break;
}

sealed class HidMaestroBroker
{
    private readonly ConcurrentDictionary<string, HidMaestroSession> _sessions = new();
    private readonly object _loadLock = new();
    private HidMaestroRuntime? _runtime;
    private string? _loadError;

    public BrokerResponse Handle(BrokerRequest request)
    {
        try
        {
            return request.Command switch
            {
                "hello" => BrokerResponse.Success(request.Id, "DSCC HIDMaestro broker ready.", available: true, supportedKinds: ["xbox360"]),
                "provider_status" => ProviderStatus(request.Id),
                "create" => Create(request),
                "destroy" => Destroy(request),
                "cleanup" => Cleanup(request.Id),
                "shutdown" => Cleanup(request.Id),
                _ => BrokerResponse.Fail(request.Id, "Unknown broker command.")
            };
        }
        catch (Exception error)
        {
            BrokerLog.Error(error.ToString());
            return BrokerResponse.Fail(request.Id, "HIDMaestro broker command failed.");
        }
    }

    public void Update(BrokerRequest request)
    {
        if (string.IsNullOrWhiteSpace(request.SessionId)) return;
        if (_sessions.TryGetValue(request.SessionId, out var session))
        {
            try
            {
                if (request.Kind != "xbox360")
                {
                    session.Submit(VirtualGamepadFrame.Neutral);
                    return;
                }
                session.Submit(VirtualGamepadFrame.TryFrom(request, out var frame)
                    ? frame
                    : VirtualGamepadFrame.Neutral);
            }
            catch (Exception error)
            {
                BrokerLog.Error(error.ToString());
                try { session.Submit(VirtualGamepadFrame.Neutral); } catch { }
            }
        }
    }

    private BrokerResponse ProviderStatus(ulong id)
    {
        if (!BrokerSecurity.IsElevated())
        {
            return BrokerResponse.Success(id, "HIDMaestro broker requires administrator privileges.", available: false);
        }
        var runtime = Runtime();
        if (runtime is null)
        {
            return BrokerResponse.Success(id, _loadError ?? "HIDMaestro.Core.dll is unavailable.", available: false);
        }
        return BrokerResponse.Success(id, "HIDMaestro broker is available.", available: true, supportedKinds: ["xbox360"]);
    }

    private BrokerResponse Create(BrokerRequest request)
    {
        if (!BrokerSecurity.IsElevated())
        {
            return BrokerResponse.Fail(request.Id, "HIDMaestro broker requires administrator privileges.");
        }
        if (request.ControllerId is null || request.Kind != "xbox360")
        {
            return BrokerResponse.Fail(request.Id, "Unsupported virtual output request.");
        }
        var runtime = Runtime();
        if (runtime is null)
        {
            return BrokerResponse.Fail(request.Id, _loadError ?? "HIDMaestro runtime unavailable.");
        }
        var sessionId = $"dscc-{Guid.NewGuid():N}";
        _sessions[sessionId] = runtime.CreateXbox360Session();
        return BrokerResponse.Success(request.Id, "HIDMaestro virtual controller created.", sessionId: sessionId, supportedKinds: ["xbox360"]);
    }

    private BrokerResponse Destroy(BrokerRequest request)
    {
        if (request.SessionId is not null && _sessions.TryRemove(request.SessionId, out var session))
        {
            session.Dispose();
        }
        return BrokerResponse.Success(request.Id, "HIDMaestro virtual controller destroyed.", available: true, supportedKinds: ["xbox360"]);
    }

    private BrokerResponse Cleanup(ulong id)
    {
        foreach (var session in _sessions.Values) session.Dispose();
        _sessions.Clear();
        return BrokerResponse.Success(id, "HIDMaestro broker cleanup complete.", available: true, supportedKinds: ["xbox360"]);
    }

    private HidMaestroRuntime? Runtime()
    {
        if (_runtime is not null || _loadError is not null) return _runtime;
        lock (_loadLock)
        {
            if (_runtime is not null || _loadError is not null) return _runtime;
            try
            {
                _runtime = HidMaestroRuntime.Load();
            }
            catch (Exception error)
            {
                BrokerLog.Error(error.ToString());
                _loadError = error.Message;
            }
            return _runtime;
        }
    }
}

static class BrokerLog
{
    public static void Error(string message)
    {
        if (Environment.GetEnvironmentVariable("DSCC_HIDMAESTRO_BROKER_DEBUG") == "1")
        {
            Console.Error.WriteLine(message);
        }
    }
}

static class BrokerSecurity
{
    public static bool IsElevated()
    {
        if (!OperatingSystem.IsWindows()) return false;
        using var identity = WindowsIdentity.GetCurrent();
        return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);
    }
}

sealed class HidMaestroRuntime
{
    private readonly object _context;
    private readonly MethodInfo _getProfile;
    private readonly MethodInfo _createController;
    private readonly MethodInfo? _catalogProfileById;
    private readonly Type _stateType;
    private readonly Type _buttonType;
    private readonly Type _hatType;
    private readonly Type _helpersType;

    private HidMaestroRuntime(
        object context,
        MethodInfo getProfile,
        MethodInfo createController,
        MethodInfo? catalogProfileById,
        Type stateType,
        Type buttonType,
        Type hatType,
        Type helpersType)
    {
        _context = context;
        _getProfile = getProfile;
        _createController = createController;
        _catalogProfileById = catalogProfileById;
        _stateType = stateType;
        _buttonType = buttonType;
        _hatType = hatType;
        _helpersType = helpersType;
    }

    public static HidMaestroRuntime Load()
    {
        var baseDir = AppContext.BaseDirectory;
        var assemblyPath = Path.Combine(baseDir, "HIDMaestro.Core.dll");
        if (!File.Exists(assemblyPath)) throw new InvalidOperationException("HIDMaestro.Core.dll was not found next to the broker.");
        var assembly = Assembly.LoadFrom(assemblyPath);
        var contextType = FindType(assembly, "HMContext");
        var stateType = FindType(assembly, "HMGamepadState");
        var buttonType = FindType(assembly, "HMButton");
        var hatType = FindType(assembly, "HMHat");
        var helpersType = FindType(assembly, "HMGamepadStateHelpers");
        var catalogType = assembly.GetTypes().FirstOrDefault(type => type.Name == "HMaestroProfileCatalog");
        var context = CreateContext(contextType);
        contextType.GetMethod("LoadDefaultProfiles", BindingFlags.Public | BindingFlags.Instance, Type.EmptyTypes)?.Invoke(context, []);
        var getProfile = RequiredMethod(contextType, "GetProfile");
        var createController = RequiredMethod(contextType, "CreateController");
        var catalogProfileById = catalogType?.GetMethods(BindingFlags.Public | BindingFlags.Static)
            .FirstOrDefault(method => method.Name == "GetProfileById");
        return new HidMaestroRuntime(context, getProfile, createController, catalogProfileById, stateType, buttonType, hatType, helpersType);
    }

    public HidMaestroSession CreateXbox360Session()
    {
        var profile = _getProfile.Invoke(_context, ["xbox-360-wired"])
            ?? _catalogProfileById?.Invoke(null, ["xbox-360-wired"])
            ?? throw new InvalidOperationException("HIDMaestro Xbox 360 profile was not found.");
        var controller = _createController.Invoke(_context, [profile])
            ?? throw new InvalidOperationException("HIDMaestro controller creation failed.");
        controller.GetType().GetMethod("Connect", Type.EmptyTypes)?.Invoke(controller, []);
        return new HidMaestroSession(controller, profile, _stateType, _buttonType, _hatType, _helpersType);
    }

    private static object CreateContext(Type contextType)
    {
        var ctor = contextType.GetConstructor(Type.EmptyTypes);
        if (ctor is not null) return ctor.Invoke([]);
        foreach (var name in new[] { "Create", "Open", "Default" })
        {
            var method = contextType.GetMethod(name, BindingFlags.Public | BindingFlags.Static, Type.EmptyTypes);
            if (method?.Invoke(null, []) is { } context) return context;
        }
        throw new InvalidOperationException("HIDMaestro context factory was not found.");
    }

    private static Type FindType(Assembly assembly, string name) =>
        assembly.GetTypes().FirstOrDefault(type => type.Name == name)
        ?? throw new InvalidOperationException($"HIDMaestro type {name} was not found.");

    private static MethodInfo RequiredMethod(Type type, string name) =>
        type.GetMethods(BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static)
            .FirstOrDefault(method => method.Name == name)
        ?? throw new InvalidOperationException($"HIDMaestro method {name} was not found.");
}

sealed class HidMaestroSession : IDisposable
{
    private readonly object _controller;
    private readonly object _profile;
    private readonly Type _stateType;
    private readonly Type _buttonType;
    private readonly Type _hatType;
    private readonly MethodInfo _submitState;
    private readonly MethodInfo? _standardAxes;
    private readonly MemberInfo? _axesMember;
    private readonly MemberInfo? _buttonsMember;
    private readonly MemberInfo? _hatMember;
    private readonly ulong _buttonA;
    private readonly ulong _buttonB;
    private readonly ulong _buttonX;
    private readonly ulong _buttonY;
    private readonly ulong _buttonDpadUp;
    private readonly ulong _buttonDpadRight;
    private readonly ulong _buttonDpadDown;
    private readonly ulong _buttonDpadLeft;
    private readonly ulong _buttonLeftShoulder;
    private readonly ulong _buttonRightShoulder;
    private readonly ulong _buttonLeftThumb;
    private readonly ulong _buttonRightThumb;
    private readonly ulong _buttonBack;
    private readonly ulong _buttonStart;
    private readonly ulong _buttonGuide;
    private readonly ulong _buttonTouchpad;
    private readonly ulong _buttonShare;
    private readonly object _hatNone;
    private readonly object _hatNorth;
    private readonly object _hatNorthEast;
    private readonly object _hatEast;
    private readonly object _hatSouthEast;
    private readonly object _hatSouth;
    private readonly object _hatSouthWest;
    private readonly object _hatWest;
    private readonly object _hatNorthWest;

    public HidMaestroSession(object controller, object profile, Type stateType, Type buttonType, Type hatType, Type helpersType)
    {
        _controller = controller;
        _profile = profile;
        _stateType = stateType;
        _buttonType = buttonType;
        _hatType = hatType;
        _submitState = controller.GetType().GetMethod("SubmitState")
            ?? throw new InvalidOperationException("HIDMaestro SubmitState method was not found.");
        _standardAxes = helpersType.GetMethods(BindingFlags.Public | BindingFlags.Static)
            .FirstOrDefault(candidate => candidate.Name == "StandardAxes" && candidate.GetParameters().Length >= 7);
        _axesMember = SettableMember(_stateType, "Axes");
        _buttonsMember = SettableMember(_stateType, "Buttons");
        _hatMember = SettableMember(_stateType, "Hat");
        _buttonA = ButtonValue("A");
        _buttonB = ButtonValue("B");
        _buttonX = ButtonValue("X");
        _buttonY = ButtonValue("Y");
        _buttonDpadUp = ButtonValue("DpadUp", "DPadUp", "Up");
        _buttonDpadRight = ButtonValue("DpadRight", "DPadRight", "Right");
        _buttonDpadDown = ButtonValue("DpadDown", "DPadDown", "Down");
        _buttonDpadLeft = ButtonValue("DpadLeft", "DPadLeft", "Left");
        _buttonLeftShoulder = ButtonValue("LeftBumper", "LeftShoulder", "LB");
        _buttonRightShoulder = ButtonValue("RightBumper", "RightShoulder", "RB");
        _buttonLeftThumb = ButtonValue("LeftStick", "LeftThumb", "LS");
        _buttonRightThumb = ButtonValue("RightStick", "RightThumb", "RS");
        _buttonBack = ButtonValue("Back", "Select");
        _buttonStart = ButtonValue("Start");
        _buttonGuide = ButtonValue("Guide", "Home");
        _buttonTouchpad = ButtonValue("Touchpad");
        _buttonShare = ButtonValue("Share", "Back");
        _hatNone = HatValue("None");
        _hatNorth = HatValue("North");
        _hatNorthEast = HatValue("NorthEast");
        _hatEast = HatValue("East");
        _hatSouthEast = HatValue("SouthEast");
        _hatSouth = HatValue("South");
        _hatSouthWest = HatValue("SouthWest");
        _hatWest = HatValue("West");
        _hatNorthWest = HatValue("NorthWest");
    }

    public void Submit(VirtualGamepadFrame frame)
    {
        var hmState = Activator.CreateInstance(_stateType)
            ?? throw new InvalidOperationException("HIDMaestro state creation failed.");
        SetMember(_axesMember, hmState, StandardAxes(frame));
        SetMember(_buttonsMember, hmState, Buttons(frame.Buttons));
        SetMember(_hatMember, hmState, Hat(frame.Buttons));
        _submitState.Invoke(_controller, [hmState]);
    }

    public void Dispose()
    {
        try { _controller.GetType().GetMethod("Disconnect", Type.EmptyTypes)?.Invoke(_controller, []); } catch { }
        if (_controller is IDisposable disposable) disposable.Dispose();
    }

    private object? StandardAxes(VirtualGamepadFrame frame)
    {
        if (_standardAxes is null) return null;
        return _standardAxes.Invoke(null, [
            _profile,
            SignedToUnit(frame.LeftX),
            SignedToUnit(frame.LeftY),
            SignedToUnit(frame.RightX),
            SignedToUnit(frame.RightY),
            Unit(frame.LeftTrigger),
            Unit(frame.RightTrigger)
        ]);
    }

    private object Buttons(uint buttons)
    {
        ulong value = 0;
        if ((buttons & VirtualButtonBits.A) != 0) value |= _buttonA;
        if ((buttons & VirtualButtonBits.B) != 0) value |= _buttonB;
        if ((buttons & VirtualButtonBits.X) != 0) value |= _buttonX;
        if ((buttons & VirtualButtonBits.Y) != 0) value |= _buttonY;
        if ((buttons & VirtualButtonBits.DpadUp) != 0) value |= _buttonDpadUp;
        if ((buttons & VirtualButtonBits.DpadRight) != 0) value |= _buttonDpadRight;
        if ((buttons & VirtualButtonBits.DpadDown) != 0) value |= _buttonDpadDown;
        if ((buttons & VirtualButtonBits.DpadLeft) != 0) value |= _buttonDpadLeft;
        if ((buttons & VirtualButtonBits.LeftShoulder) != 0) value |= _buttonLeftShoulder;
        if ((buttons & VirtualButtonBits.RightShoulder) != 0) value |= _buttonRightShoulder;
        if ((buttons & VirtualButtonBits.LeftThumb) != 0) value |= _buttonLeftThumb;
        if ((buttons & VirtualButtonBits.RightThumb) != 0) value |= _buttonRightThumb;
        if ((buttons & VirtualButtonBits.Back) != 0) value |= _buttonBack;
        if ((buttons & VirtualButtonBits.Start) != 0) value |= _buttonStart;
        if ((buttons & VirtualButtonBits.Guide) != 0) value |= _buttonGuide;
        if ((buttons & VirtualButtonBits.Touchpad) != 0) value |= _buttonTouchpad;
        if ((buttons & VirtualButtonBits.Share) != 0) value |= _buttonShare;
        return Enum.ToObject(_buttonType, value);
    }

    private object Hat(uint buttons)
    {
        var up = (buttons & VirtualButtonBits.DpadUp) != 0;
        var right = (buttons & VirtualButtonBits.DpadRight) != 0;
        var down = (buttons & VirtualButtonBits.DpadDown) != 0;
        var left = (buttons & VirtualButtonBits.DpadLeft) != 0;

        if (up == down)
        {
            up = false;
            down = false;
        }
        if (left == right)
        {
            left = false;
            right = false;
        }

        return (up, right, down, left) switch
        {
            (true, true, false, false) => _hatNorthEast,
            (true, false, false, true) => _hatNorthWest,
            (false, true, true, false) => _hatSouthEast,
            (false, false, true, true) => _hatSouthWest,
            (true, false, false, false) => _hatNorth,
            (false, true, false, false) => _hatEast,
            (false, false, true, false) => _hatSouth,
            (false, false, false, true) => _hatWest,
            _ => _hatNone
        };
    }

    private ulong ButtonValue(params string[] names)
    {
        foreach (var name in names)
        {
            try
            {
                return Convert.ToUInt64(Enum.Parse(_buttonType, name, ignoreCase: true));
            }
            catch { }
        }
        return 0;
    }

    private object HatValue(string name) => Enum.Parse(_hatType, name, ignoreCase: true);

    private static MemberInfo? SettableMember(Type type, string name)
    {
        var property = type.GetProperty(name, BindingFlags.Public | BindingFlags.Instance);
        if (property is not null && property.CanWrite) return property;
        return type.GetField(name, BindingFlags.Public | BindingFlags.Instance);
    }

    private static void SetMember(MemberInfo? member, object target, object? value)
    {
        if (member is null || value is null) return;
        if (member is PropertyInfo property)
        {
            property.SetValue(target, value);
            return;
        }
        ((FieldInfo)member).SetValue(target, value);
    }

    private static float Unit(float value) => Math.Clamp(value, 0.0f, 1.0f);
    private static float SignedToUnit(float value) => (Math.Clamp(value, -1.0f, 1.0f) + 1.0f) * 0.5f;
}

sealed record BrokerRequest(
    string Protocol,
    ulong Id,
    string Command,
    string? ControllerId,
    string? SessionId,
    string? Kind,
    int? Lx,
    int? Ly,
    int? Rx,
    int? Ry,
    int? Lt,
    int? Rt,
    uint? Buttons);

sealed record BrokerResponse(
    ulong Id,
    bool Ok,
    bool? Available,
    string? Message,
    string? SessionId,
    string[] SupportedKinds)
{
    public static BrokerResponse Success(
        ulong id,
        string message,
        bool? available = null,
        string? sessionId = null,
        string[]? supportedKinds = null) =>
        new(id, true, available, message, sessionId, supportedKinds ?? []);

    public static BrokerResponse Fail(ulong id, string message) =>
        new(id, false, false, message, null, []);
}

readonly record struct VirtualGamepadFrame(
    float LeftX,
    float LeftY,
    float RightX,
    float RightY,
    float LeftTrigger,
    float RightTrigger,
    uint Buttons)
{
    private const float AxisWireMax = 32767.0f;
    private const float TriggerWireMax = 65535.0f;

    public static readonly VirtualGamepadFrame Neutral = new(0, 0, 0, 0, 0, 0, 0);

    public static bool TryFrom(BrokerRequest request, out VirtualGamepadFrame frame)
    {
        if (HasCompactUpdateFields(request))
        {
            return TryFromCompact(request, out frame);
        }
        frame = Neutral;
        return false;
    }

    private static bool TryFromCompact(BrokerRequest request, out VirtualGamepadFrame frame)
    {
        frame = Neutral;
        if (!TryAxis(request.Lx, out var lx)
            || !TryAxis(request.Ly, out var ly)
            || !TryAxis(request.Rx, out var rx)
            || !TryAxis(request.Ry, out var ry)
            || !TryTrigger(request.Lt, out var lt)
            || !TryTrigger(request.Rt, out var rt)
            || request.Buttons is not { } buttons
            || (buttons & ~VirtualButtonBits.KnownMask) != 0)
        {
            return false;
        }
        frame = new(lx, ly, rx, ry, lt, rt, buttons);
        return true;
    }

    private static bool HasCompactUpdateFields(BrokerRequest request) =>
        request.Lx.HasValue
        || request.Ly.HasValue
        || request.Rx.HasValue
        || request.Ry.HasValue
        || request.Lt.HasValue
        || request.Rt.HasValue
        || request.Buttons.HasValue;

    private static bool TryAxis(int? value, out float axis)
    {
        axis = 0;
        if (value is null || value < -32767 || value > 32767) return false;
        axis = value.Value / AxisWireMax;
        return true;
    }

    private static bool TryTrigger(int? value, out float trigger)
    {
        trigger = 0;
        if (value is null || value < 0 || value > 65535) return false;
        trigger = value.Value / TriggerWireMax;
        return true;
    }
}

static class VirtualButtonBits
{
    public const uint A = 1u << 0;
    public const uint B = 1u << 1;
    public const uint X = 1u << 2;
    public const uint Y = 1u << 3;
    public const uint DpadUp = 1u << 4;
    public const uint DpadRight = 1u << 5;
    public const uint DpadDown = 1u << 6;
    public const uint DpadLeft = 1u << 7;
    public const uint LeftShoulder = 1u << 8;
    public const uint RightShoulder = 1u << 9;
    public const uint LeftThumb = 1u << 10;
    public const uint RightThumb = 1u << 11;
    public const uint Back = 1u << 12;
    public const uint Start = 1u << 13;
    public const uint Guide = 1u << 14;
    public const uint Touchpad = 1u << 15;
    public const uint Share = 1u << 16;
    public const uint KnownMask = A | B | X | Y | DpadUp | DpadRight | DpadDown | DpadLeft
        | LeftShoulder | RightShoulder | LeftThumb | RightThumb | Back | Start | Guide | Touchpad | Share;

}

static class JsonOptions
{
    public static readonly JsonSerializerOptions Default = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        PropertyNameCaseInsensitive = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
    };
}
